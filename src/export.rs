/// Quote every field and prevent spreadsheet formula execution on explicit CSV export.
pub fn csv_field(value: &str) -> String {
    let prefix = if value.trim_start().starts_with(['=', '+', '-', '@'])
        || value.starts_with(['\t', '\r'])
    {
        "'"
    } else {
        ""
    };
    format!("\"{}{}\"", prefix, value.replace('"', "\"\""))
}
