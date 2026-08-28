pub mod fst_dict;
pub mod cin_parser;
pub mod entry;
pub mod builder;

// 重新导出常用类型
pub use entry::{CinEntry, DictEntry, DictSource, DictBuilderConfig};
pub use cin_parser::CinParser;
pub use fst_dict::{FstDict, FstDictBuilder, edit_distance};
pub use builder::{DictBuilder, DictValidator};