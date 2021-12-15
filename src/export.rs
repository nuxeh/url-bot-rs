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
    let string = serde_json::ser::to_string_pretty(&set)?
        .replace(":", " =")
        .replace(",", "");

    // add semi-colons, except within arrays (with a very cludgy parser)
    let mut in_array = false;
    for mut l in string.lines() {
        if l.contains("= [") { in_array = true };
        if l.contains("]") { in_array = false };
        if !in_array {
            l.push(";");
        }
    }

    Ok(string)
}

#[cfg(test)]
mod tests {
    use super::*;

}
