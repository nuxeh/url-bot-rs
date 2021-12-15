use crate::{
    config::{Conf, ConfSet},
    cli::url_bot_rs::ExportFormat
};
use failure::Error;

pub fn export(configs: &[Conf], format: ExportFormat) -> Result<String, Error> {
    let set = ConfSet::from_slice(configs);

    match format {
        ExportFormat::Json => Ok(serde_json::ser::to_string_pretty(&set)?),
        ExportFormat::Toml => Ok(toml::ser::to_string(&set)?),
        ExportFormat::Nix => Ok(serialise_nix(&set)?),
    }
}

fn serialise_nix(set: &ConfSet) -> Result<String, Error> {
    let mut string = serde_json::ser::to_string_pretty(&set)?
        .replace(":", " =")
        .replace(",", "");

    // add semi-colons, except within arrays (with a very cludgy parser)
    string = string.lines()
        .fold((String::new(), false), |(mut s, mut in_array), l| {
            s = s + l.trim_end();
            if l.contains("= [") { in_array = true };
            if l.contains("]") { in_array = false };
            if !in_array && l.chars().last() != Some('{') {
                s.push(';');
            };
            s.push('\n');
            (s, in_array)
        }).0;

    Ok(string)
}

#[cfg(test)]
mod tests {
    use super::*;

}
