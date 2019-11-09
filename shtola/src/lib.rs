use pathdiff::diff_paths;
use std::default::Default;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use walkdir::WalkDir;

pub use im::HashMap;
pub use ware::Ware;
pub use yaml_rust::Yaml;

mod frontmatter;

pub struct Shtola {
	ware: Ware<IR>,
	ir: IR,
}

impl Shtola {
	pub fn new() -> Shtola {
		let config: Config = Default::default();
		let ir = IR {
			files: HashMap::new(),
			config,
		};
		Shtola {
			ware: Ware::new(),
			ir,
		}
	}

	pub fn ignores(&mut self, vec: &mut Vec<PathBuf>) {
		self.ir.config.ignores.append(vec);
		self.ir.config.ignores.dedup();
	}

	pub fn source<T: Into<PathBuf>>(&mut self, path: T) {
		self.ir.config.source = fs::canonicalize(path.into()).unwrap();
	}

	pub fn destination<T: Into<PathBuf> + Clone>(&mut self, path: T) {
		fs::create_dir_all(path.clone().into()).expect("Unable to create destination directory!");
		self.ir.config.destination = fs::canonicalize(path.into()).unwrap();
	}

	pub fn clean(&mut self, b: bool) {
		self.ir.config.clean = b;
	}

	pub fn frontmatter(&mut self, b: bool) {
		self.ir.config.frontmatter = b;
	}

	pub fn register(&mut self, func: Box<dyn Fn(IR) -> IR>) {
		self.ware.wrap(func);
	}

	pub fn build(&mut self) -> Result<IR, std::io::Error> {
		if self.ir.config.clean {
			fs::remove_dir_all(&self.ir.config.destination)?;
			fs::create_dir_all(&self.ir.config.destination).expect("Unable to recreate destination directory!");
		}
		let files = read_dir(&self.ir.config.source)?;
		self.ir.files = files;
		let result_ir = self.ware.run(self.ir.clone());
		write_dir(result_ir.clone(), &self.ir.config.destination)?;
		Ok(result_ir)
	}
}

#[derive(Debug, Clone)]
pub struct IR {
	files: HashMap<PathBuf, ShFile>,
	config: Config,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
	ignores: Vec<PathBuf>,
	source: PathBuf,
	destination: PathBuf,
	clean: bool,
	frontmatter: bool,
}

#[derive(Debug, Clone)]
pub struct ShFile {
	frontmatter: Vec<Yaml>,
	content: Vec<u8>,
}

fn read_dir(source: &PathBuf) -> Result<HashMap<PathBuf, ShFile>, std::io::Error> {
	let mut result = HashMap::new();
	let iters = WalkDir::new(source)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| !e.path().is_dir());
	for entry in iters {
		let path = entry.path();
		let mut content = String::new();
		fs::File::open(path)?.read_to_string(&mut content)?;
		let (matter, content) = frontmatter::lexer(&content);
		let yaml = frontmatter::to_yaml(&matter);
		let file = ShFile {
			frontmatter: yaml,
			content: content.into(),
		};
		let rel_path = diff_paths(path, source).unwrap();
		result.insert(rel_path, file);
	}
	Ok(result)
}

fn write_dir(ir: IR, dest: &PathBuf) -> Result<(), std::io::Error> {
	for (path, file) in ir.files {
		let dest_path = dest.join(path);
		fs::create_dir_all(dest_path.parent().unwrap()).expect("Unable to create destination subdirectory!");
		fs::File::create(dest_path)?.write_all(&file.content)?;
	}
	Ok(())
}

#[test]
fn read_works() {
	let mut s = Shtola::new();
	s.source("../fixtures/simple");
	s.destination("./");
	let r = s.build().unwrap();
	assert_eq!(r.files.len(), 1);
	let keys: Vec<&PathBuf> = r.files.keys().collect();
	assert_eq!(keys[0].to_str().unwrap(), "hello.txt");
}

#[test]
fn clean_works() {
	let mut s = Shtola::new();
	s.source("../fixtures/simple");
	s.destination("../fixtures/dest_clean");
	s.clean(true);
	fs::create_dir_all("../fixtures/dest_clean").unwrap();
	fs::write("../fixtures/dest_clean/blah.foo", "").unwrap();
	s.build().unwrap();
	let fpath = PathBuf::from("../fixtures/dest_clean/blah.foo");
	assert_eq!(fpath.exists(), false);
}

#[test]
fn write_works() {
	let mut s = Shtola::new();
	s.source("../fixtures/simple");
	s.destination("../fixtures/dest");
	s.clean(true);
	let mw = Box::new(|ir: IR| {
		let mut update_hash: HashMap<PathBuf, ShFile> = HashMap::new();
		for (k, v) in &ir.files {
			update_hash.insert(k.into(), ShFile {
				frontmatter: v.frontmatter.clone(),
				content: "hello".into(),
			});
		}
		IR { files: update_hash.union(ir.files), ..ir }
	});
	s.register(mw);
	s.build().unwrap();
	let dpath = PathBuf::from("../fixtures/dest/hello.txt");
	assert!(dpath.exists());
	let file = &fs::read(dpath).unwrap();
	let fstring = String::from_utf8_lossy(file);
	assert_eq!(fstring, "hello");
}