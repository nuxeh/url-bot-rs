use crate::{
    config::{Conf, ConfSet},
    cli::url_bot_rs::ExportFormat
};
use serde_json::Error;

pub fn export(configs: &[Conf], format: ExportFormat) -> Result<String, Error> {
    let set = ConfSet::from_slice(configs);

    Ok(
        serde_json::ser::to_string_pretty(&set)?
        .replace(":", " =")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

}
