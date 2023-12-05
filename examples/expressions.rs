use bedder::intersection::{Intersection, Intersections};
use mlua::prelude::*;
use std::fs;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

use bedder::position::Position;
use bedder::sniff;
use clap::Parser;
extern crate bedder;
use crate::bedder::chrom_ordering::parse_genome;
use crate::bedder::intersection::IntersectionIterator;

#[derive(Parser, Debug)]
struct Args {
    a: PathBuf,
    b: PathBuf,

    fai: PathBuf,
    #[clap(short, long, help = "Lua format string")]
    format: Option<String>,
}

/// The Bs that overlap A.
/// We use this so we can add custom methods to the overlaps.
struct BS(Vec<Arc<Position>>);

impl mlua::UserData for BS {}

fn wrap_position(lua: &Lua) -> LuaResult<()> {
    lua.register_userdata_type::<Arc<Position>>(|lp| {
        lp.add_field_method_get("chromosome", |_, this| Ok(this.chrom().to_string()));
        lp.add_field_method_get("start", |_, this| Ok(this.start()));
        lp.add_field_method_get("stop", |_, this| Ok(this.stop()));
    })?;

    // cargo run --example expressions LCR-hs38.bed.gz LCR-hs38.bed.gz $fai
    // --format "return \`{a.chromosome}\t{a.start}\t{a.stop}\t{bs.length}\t{bs:bases_overlapping()}\`"
    lua.register_userdata_type::<BS>(|bs| {
        bs.add_field_method_get("length", |_, this| Ok(this.0.len()));
        bs.add_method("bases_overlapping", |_, this, ()| {
            Ok(this.0.iter().map(|p| p.stop() - p.start()).sum::<u64>())
        });
    })?;

    lua.register_userdata_type::<Intersections>(|inter| {
        inter.add_field_method_get("base", |lua, this| {
            lua.create_any_userdata(this.base_interval.clone())
        });
        inter.add_field_method_get("overlapping", |lua, this| {
            lua.create_any_userdata(
                this.overlapping
                    .iter()
                    .map(|inter| Arc::clone(&inter.interval))
                    .collect::<Vec<_>>(),
            )
        });
    })
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // sniff determines the file type (bam/cram/bcf/vcf/bed/gff/gtf)
    // and returns a PositionIterator
    let ai = sniff::open_file(&args.a)?;
    let bi = sniff::open_file(&args.b)?;
    let lua = Lua::new();

    let lua_fun = if let Some(expr) = args.format {
        match wrap_position(&lua) {
            Ok(_) => {}
            Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
        }
        //let e2 = "return `{intersection.base.chromosome}:{intersection.base.start}-{intersection.base.stop}`";

        let lua_fun = match lua.load(expr).into_function() {
            Ok(f) => f,
            Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
        };
        // lua.register_userdata_type::<Position, _>(|p| p.add_field_method_);
        Some(lua_fun)
    } else {
        None
    };

    // bedder always requires a hashmap that indicates the chromosome order
    let fh = BufReader::new(fs::File::open(args.fai)?);
    let h = parse_genome(fh)?;

    // we can have any number of b (other_iterators).
    let it = IntersectionIterator::new(ai, vec![bi], &h)?;

    // we need to use buffered stdout or performance is determined by
    // file IO
    let mut stdout = BufWriter::new(io::stdout().lock());
    let globals = lua.globals();

    for intersection in it {
        let intersection = intersection?;

        if let Some(f) = &lua_fun {
            let r = lua.scope(|scope| {
                let a = intersection.base_interval.clone();
                let user_data_a = scope.create_any_userdata(a)?;
                globals.set("a", user_data_a)?;
                let bs = BS(intersection
                    .overlapping
                    .iter()
                    .map(|inter: &Intersection| Arc::clone(&inter.interval))
                    .collect::<Vec<_>>());

                globals.set("bs", bs)?;

                f.call::<_, String>(())
            });

            match r {
                Ok(s) => writeln!(&mut stdout, "{}", s)?,
                Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
            }
        }
    }

    Ok(())
}
