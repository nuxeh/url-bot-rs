use crate::{
    config::{Conf, ConfSet},
    cli::url_bot_rs::ExportFormat
};


pub fn export(configs: &[Conf], format: ExportFormat) -> String {

    String::from("foo")
}

#[cfg(test)]
mod tests {
    use super::*;

}
