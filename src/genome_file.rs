use crate::string::String;
use hashbrown::HashMap;
use std::io::{self, BufRead, Read};

pub fn parse_genome<R>(reader: R) -> io::Result<HashMap<String, usize>>
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
        let chrom = fields
            .next()
            .expect("require at least one column in genome file");
        genome.insert(String::from(chrom), genome.len());
        line.clear();
    }
    Ok(genome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_genome() {
        let genome_str = "chr1\nchr2\nchr3\n";
        let genome = parse_genome(genome_str.as_bytes()).unwrap();
        assert_eq!(genome.len(), 3);
        assert_eq!(genome.get("chr1"), Some(&0));
        assert_eq!(genome.get("chr2"), Some(&1));
        assert_eq!(genome.get("chr3"), Some(&2));
    }
}
