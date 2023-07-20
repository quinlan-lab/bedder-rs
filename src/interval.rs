use crate::position::{Field, FieldError, Positioned, Value};
use crate::string::String;
/// Interval type is a simple struct that can be used as a default interval type.
/// It has a chromosome, start, and stop field along with a (linear) HashMap of Values.
use linear_map::LinearMap;
use std::fmt::Debug;

#[derive(Debug, Default)]
pub struct Interval {
    pub chrom: String,
    pub start: u64,
    pub stop: u64,
    pub fields: LinearMap<String, Value>,
}

impl Positioned for Interval {
    #[inline]
    fn start(&self) -> u64 {
        self.start
    }
    #[inline]
    fn stop(&self) -> u64 {
        self.stop
    }
    #[inline]
    fn chrom(&self) -> &str {
        &self.chrom
    }

    #[inline]
    fn value(&self, f: Field) -> Result<Value, FieldError> {
        match f {
            Field::String(name) => match self.fields.get(&name) {
                None => Err(FieldError::InvalidFieldName(name)),
                Some(v) => match v {
                    Value::Strings(s) => Ok(Value::Strings(s.clone())),
                    Value::Ints(i) => Ok(Value::Ints(i.clone())),
                    Value::Floats(f) => Ok(Value::Floats(f.clone())),
                },
            },
            Field::Int(i) => {
                let name = self.fields.keys().nth(i);
                match name {
                    None => Err(FieldError::InvalidFieldIndex(i)),
                    Some(name) => match self.fields.get(name) {
                        None => Err(FieldError::InvalidFieldName(name.clone())),
                        Some(v) => match v {
                            Value::Strings(s) => Ok(Value::Strings(s.clone())),
                            Value::Ints(i) => Ok(Value::Ints(i.clone())),
                            Value::Floats(f) => Ok(Value::Floats(f.clone())),
                        },
                    },
                }
            }
        }
    }
}
