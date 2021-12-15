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
    Ok(
        serde_json::ser::to_string_pretty(&set)?
        .replace(":", " =")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

}
