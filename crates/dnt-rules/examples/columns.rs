//! List the class-table variables of every class in a source.
//!
//! cargo run -p dnt-rules --example columns -- /path/to/dnd-data XPHB

use dnt_data::{Kind, Library};
use dnt_rules::class_table::class_columns;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: columns <data dir> [SOURCE]");
    let source = args.next().unwrap_or_else(|| "XPHB".into());
    let lib = Library::load_data_dir(path.as_ref()).expect("load");
    for e in lib
        .entities()
        .iter()
        .filter(|e| e.kind == Kind::Class && e.source.eq_ignore_ascii_case(&source))
    {
        let cols: Vec<String> = class_columns(&e.data)
            .iter()
            .filter(|c| !c.var.contains(".slots."))
            .map(|c| format!("{} [1:{} 20:{}]", c.var, c.at(1), c.at(20)))
            .collect();
        println!("{}: {}", e.name, cols.join(", "));
    }
}
