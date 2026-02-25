/// Format mapped numeric output:
/// - render integral finite values without a decimal suffix
/// - otherwise preserve Rust's default floating-point rendering
pub fn format_map_number(v: f64) -> String {
    if v.is_finite() && v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}
