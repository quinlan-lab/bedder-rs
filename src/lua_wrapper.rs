use mlua::prelude::*;
use simplebed::BedRecord as SimpleBedRecord;

use crate::intersections::{IntersectionMode, IntersectionPart, OverlapAmount};
use crate::position::Position;
use std::sync::Arc;

// Wrapper for simplebed::BedRecord
pub struct LuaBedRecord {
    inner: SimpleBedRecord,
}

impl LuaBedRecord {
    fn new(inner: SimpleBedRecord) -> Self {
        LuaBedRecord { inner }
    }
}

// Wrapper for Position
pub struct LuaPosition {
    inner: Arc<Position>,
}

impl LuaPosition {
    pub fn new(inner: Arc<Position>) -> Self {
        LuaPosition { inner }
    }
}

// Wrapper for Intersections
pub struct LuaIntersections {
    inner: crate::intersection::Intersections,
}

impl LuaIntersections {
    pub fn new(inner: crate::intersection::Intersections) -> Self {
        LuaIntersections { inner }
    }
}

// Wrapper for Report
pub struct LuaReport {
    inner: crate::report::Report,
}

impl LuaReport {
    fn new(inner: crate::report::Report) -> Self {
        LuaReport { inner }
    }
}

// Wrapper for ReportFragment
pub struct LuaReportFragment {
    inner: crate::report::ReportFragment,
}

impl LuaReportFragment {
    fn new(inner: crate::report::ReportFragment) -> Self {
        LuaReportFragment { inner }
    }
}

impl mlua::UserData for LuaBedRecord {}
impl mlua::UserData for LuaPosition {}
impl mlua::UserData for LuaIntersections {}
impl mlua::UserData for LuaReport {}
impl mlua::UserData for LuaReportFragment {}

/// Register all Lua types
pub fn register_types(lua: &Lua) -> mlua::Result<()> {
    // Register BedRecord
    lua.register_userdata_type::<LuaBedRecord>(|reg| {
        reg.add_field_method_get("chrom", |_, this| Ok(this.inner.chrom().to_string()));
        reg.add_field_method_get("start", |_, this| Ok(this.inner.start()));
        reg.add_field_method_get("stop", |_, this| Ok(this.inner.end()));
        reg.add_field_method_get("name", |_, this| {
            Ok(this.inner.name().map(|s| s.to_string()))
        });
        reg.add_field_method_get("score", |_, this| Ok(this.inner.score()));
        reg.add_method("other_fields", |_, this, ()| {
            Ok(this
                .inner
                .other_fields()
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>())
        });
    })?;

    // Register Position
    lua.register_userdata_type::<LuaPosition>(|reg| {
        reg.add_field_method_get("chrom", |_, this| Ok(this.inner.chrom().to_string()));
        reg.add_field_method_get("start", |_, this| Ok(this.inner.start()));
        reg.add_field_method_get("stop", |_, this| Ok(this.inner.stop()));
        reg.add_method("bed", |_lua, this, ()| {
            if let Position::Bed(b) = &*this.inner {
                Ok(Some(LuaBedRecord::new(b.0.clone())))
            } else {
                Ok(None)
            }
        });
        reg.add_meta_method(LuaMetaMethod::Index, |_, _this, key: String| {
            Err::<LuaValue, _>(mlua::Error::RuntimeError(format!(
                "no index operator for {}",
                key
            )))
        });
    })?;

    // Register Intersections
    lua.register_userdata_type::<LuaIntersections>(|reg| {
        reg.add_field_method_get("base_interval", |lua, this| {
            let l = LuaPosition::new(this.inner.base_interval.clone());
            lua.create_any_userdata(l)
        });
        reg.add_field_method_get("n_overlapping", |_, this| Ok(this.inner.overlapping.len()));
        reg.add_method("overlapping", |lua, this, ()| {
            let overlapping = this
                .inner
                .overlapping
                .iter()
                .map(|i| LuaPosition::new(Arc::new(i.interval.as_ref().clone())))
                .collect::<Vec<_>>();

            let table = lua.create_table()?;
            for (i, pos) in overlapping.into_iter().enumerate() {
                table.raw_set(i + 1, lua.create_userdata(pos)?)?;
            }
            Ok(table)
        });

        #[allow(clippy::type_complexity)] // TODO: fix this later
        reg.add_method(
            "report",
            |lua,
             this,
             (a_mode, b_mode, a_part, b_part, a_requirements, b_requirements): (
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            )| {
                let a_mode = a_mode
                    .map(|s| IntersectionMode::from(s.as_str()))
                    .unwrap_or_default();
                let b_mode = b_mode
                    .map(|s| IntersectionMode::from(s.as_str()))
                    .unwrap_or_default();
                let a_part = a_part
                    .map(|s| match s.as_str() {
                        "none" => IntersectionPart::None,
                        "part" => IntersectionPart::Part,
                        "whole" => IntersectionPart::Whole,
                        "inverse" => IntersectionPart::Inverse,
                        _ => IntersectionPart::Whole,
                    })
                    .unwrap_or(IntersectionPart::Whole);
                let b_part = b_part
                    .map(|s| match s.as_str() {
                        "none" => IntersectionPart::None,
                        "part" => IntersectionPart::Part,
                        "whole" => IntersectionPart::Whole,
                        "inverse" => IntersectionPart::Inverse,
                        _ => IntersectionPart::Whole,
                    })
                    .unwrap_or(IntersectionPart::Whole);
                let a_requirements = a_requirements
                    .map(|s| OverlapAmount::from(s.as_str()))
                    .unwrap_or(OverlapAmount::Bases(1));
                let b_requirements = b_requirements
                    .map(|s| OverlapAmount::from(s.as_str()))
                    .unwrap_or(OverlapAmount::Bases(1));

                let report = this.inner.report(
                    &a_mode,
                    &b_mode,
                    &a_part,
                    &b_part,
                    &a_requirements,
                    &b_requirements,
                );
                lua.create_userdata(LuaReport::new(report))
            },
        );
    })?;

    // Register Report
    lua.register_userdata_type::<LuaReport>(|reg| {
        reg.add_method("count_overlaps_by_id", |_, this, ()| {
            Ok(this.inner.count_overlaps_by_id())
        });
        reg.add_method("count_bases_by_id", |_, this, ()| {
            Ok(this.inner.count_bases_by_id())
        });
        reg.add_method("__len", |_, this, ()| Ok(this.inner.len()));
        reg.add_method("__index", |lua, this, index: usize| {
            if index == 0 || index > this.inner.len() {
                return Err(mlua::Error::RuntimeError("index out of bounds".to_string()));
            }
            lua.create_userdata(LuaReportFragment::new(this.inner[index - 1].clone()))
        });
    })?;

    // Register ReportFragment
    lua.register_userdata_type::<LuaReportFragment>(|reg| {
        reg.add_field_method_get("id", |_, this| Ok(this.inner.id));
        reg.add_method("a", |lua, this, ()| match &this.inner.a {
            Some(pos) => Ok(Some(
                lua.create_userdata(LuaPosition::new(Arc::new(pos.clone())))?,
            )),
            None => Ok(None),
        });
        reg.add_method("b", |lua, this, ()| {
            let positions = this
                .inner
                .b
                .iter()
                .map(|pos| LuaPosition::new(Arc::new(pos.clone())))
                .collect::<Vec<_>>();

            let table = lua.create_table()?;
            for (i, pos) in positions.into_iter().enumerate() {
                table.raw_set(i + 1, lua.create_userdata(pos)?)?;
            }
            Ok(table)
        });
    })?;

    Ok(())
}

/// A compiled Lua expression that can be reused for better performance
pub struct CompiledLua {
    lua: Lua,
    chunk: mlua::Function,
}

impl CompiledLua {
    pub fn new(code: &str) -> mlua::Result<Self> {
        let lua = Lua::new();
        log::trace!("registering types");
        register_types(&lua)?;
        let chunk = lua.load(code).set_name("user-code").into_function()?;
        Ok(CompiledLua { lua, chunk })
    }

    pub fn eval(&self, intersections: crate::intersection::Intersections) -> mlua::Result<String> {
        let intersections = LuaIntersections::new(intersections);
        let ud = self.lua.create_any_userdata(intersections)?;
        self.lua.globals().set("intersection", ud)?;
        self.chunk.call::<String>(())
        /*
        self.lua.scope(|scope| {
            let ud = scope.create_any_userdata_ref(&intersections)?;
            self.lua.globals().set("intersection", ud)?;
            self.chunk.call::<String>(())
        })
        */
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intersection::Intersections;
    use crate::position::Position;
    use simplebed::BedRecord;

    #[test]
    fn test_lua_simple_expression() {
        // Create a simple BED record for testing
        let base = BedRecord::new("chr1".into(), 100, 200, None, None, vec![]);
        let overlapping = BedRecord::new("chr1".into(), 150, 250, None, None, vec![]);

        // Create an Intersections object
        let intersection = Intersections {
            base_interval: Arc::new(Position::Bed(crate::bedder_bed::BedRecord(base))),
            overlapping: vec![crate::intersection::Intersection {
                interval: Arc::new(Position::Bed(crate::bedder_bed::BedRecord(overlapping))),
                id: 0,
            }],
        };

        // Create and test a simple Lua expression
        let lua_code = "return string.format('%s\\t%d\\t%d', intersection.base_interval.chrom, intersection.base_interval.start, intersection.base_interval.stop)";
        let compiled = CompiledLua::new(lua_code).expect("failed to compile Lua code");
        let result = compiled
            .eval(intersection)
            .expect("failed to evaluate Lua code");

        assert_eq!(result, "chr1\t100\t200");
    }
}
