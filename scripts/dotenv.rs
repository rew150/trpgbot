//! ```cargo
//! [dependencies]
//! toml = "0.8"
//! ```

use std::fs;
use toml::Table;

const PATH: &str = "config/config.toml";
const ENV_PATH: &str = ".env";

fn main() {
    let toml_config = fs::read_to_string(PATH)
        .expect(&format!("could not read cfg file at {PATH}"));

    let toml_table = toml_config.parse::<Table>()
        .expect("could not read config file as toml");

    let dbstr = toml_table["sqlite_conn"].as_str()
        .expect("could not read sqlite_conn");

    fs::write(ENV_PATH, format!("DATABASE_URL='{dbstr}'\n"))
        .expect("could not write to file");

    println!("dotenv created at {ENV_PATH}");
}
