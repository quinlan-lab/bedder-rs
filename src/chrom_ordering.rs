use crate::string::String;
use hashbrown::HashMap;
use std::io::{self, BufRead, Read};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Chromosome {
    pub(crate) index: usize,
    pub(crate) length: Option<usize>,
}
/// A genome is a map from chromosome name to index with an optional chromosome length.

pub fn parse_genome<R>(reader: R) -> io::Result<HashMap<String, Chromosome>>
where
    R: Read,
{
    let mut reader = io::BufReader::new(reader);
    let mut genome = HashMap::default();
    let mut line = std::string::String::new();
    while reader.read_line(&mut line)? > 0 {
        if line.trim().is_empty() || line.starts_with('#') {
            line.clear();
            continue;
        }
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some(chrom) => {
                let length = fields.next().map(|s| s.parse::<usize>());
                let l = length.and_then(|c| match c {
                    Ok(l) => Some(l),
                    Err(_) => {
                        log::warn!(
                            "invalid length for chromosome {} with line: {}",
                            chrom,
                            line
                        );
                        None
                    }
                });
                genome.insert(
                    String::from(chrom),
                    Chromosome {
                        index: genome.len(),
                        length: l,
                    },
                );
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid genome file line: {}", line),
                ))
            }
        }
        //.expect("require at least one column in genome file");
        line.clear();
    }
    Ok(genome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_genome() {
        let genome_str = "chr1\nchr2\t43\nchr3\n";
        let genome = parse_genome(genome_str.as_bytes()).unwrap();
        assert_eq!(genome.len(), 3);
        assert_eq!(
            genome.get("chr1"),
            Some(&Chromosome {
                index: 0,
                length: None
            })
        );
        assert_eq!(
            genome.get("chr2"),
            Some(&Chromosome {
                index: 1,
                length: Some(43)
            })
        );
        assert_eq!(
            genome.get("chr3"),
            Some(&Chromosome {
                index: 2,
                length: None
            })
        );
    }
}
