//! Js module.

use crate::js::loader::{CoreModuleLoader, FsModuleLoader, ModuleLoader};
use crate::js::JsRuntime;
use crate::res::AnyResult;

use ahash::AHashMap as HashMap;
use std::cell::RefCell;
use std::collections::LinkedList;
use std::rc::Rc;
use std::sync::OnceLock;
use tracing::trace;
// use url::Url;

/// Creates v8 script origins.
pub fn create_origin<'s>(
  scope: &mut v8::HandleScope<'s, ()>,
  name: &str,
  is_module: bool,
) -> v8::ScriptOrigin<'s> {
  let name = v8::String::new(scope, name).unwrap();
  let source_map = v8::undefined(scope);

  v8::ScriptOrigin::new(
    scope,
    name.into(),
    0,
    0,
    false,
    0,
    Some(source_map.into()),
    false,
    false,
    is_module,
    None,
  )
}

#[allow(non_snake_case)]
pub fn CORE_MODULES() -> &'static HashMap<&'static str, &'static str> {
  static VALUE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
  VALUE.get_or_init(|| {
    let modules = vec![
      ("rsvim:ext/infra", include_str!("./runtime/00__infra.js")),
      // ("console", include_str!("./js/console.js")),
      // ("events", include_str!("./js/events.js")),
      // ("process", include_str!("./js/process.js")),
      // ("timers", include_str!("./js/timers.js")),
      // ("assert", include_str!("./js/assert.js")),
      // ("util", include_str!("./js/util.js")),
      // ("fs", include_str!("./module/fs.js")),
      // ("perf_hooks", include_str!("./js/perf-hooks.js")),
      // ("colors", include_str!("./js/colors.js")),
      // ("dns", include_str!("./js/dns.js")),
      // ("net", include_str!("./js/net.js")),
      // ("test", include_str!("./js/test.js")),
      // ("stream", include_str!("./js/stream.js")),
      // ("http", include_str!("./js/http.js")),
      // ("@web/abort", include_str!("./js/abort-controller.js")),
      // ("@web/text_encoding", include_str!("./js/text-encoding.js")),
      // ("@web/clone", include_str!("./js/structured-clone.js")),
      // ("@web/fetch", include_str!("./js/fetch.js")),
    ];
    HashMap::from_iter(modules)
  })
}

/// Module path on local file system.
pub type ModulePath = String;

/// Module source code.
pub type ModuleSource = String;

#[derive(Debug, Clone)]
/// Import kind.
pub enum ImportKind {
  // Loading static imports.
  Static,
  // Loading a dynamic import.
  Dynamic(v8::Global<v8::PromiseResolver>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Module import status.
///
/// NOTE: All modules (plugins/packages) will be local files on user's operating system, no
/// network/http modules will be fetching. The only one use case of `Resolving` status should be
/// dynamically import and its `Promise`.
pub enum ModuleStatus {
  // Indicates the module **itself** is being fetched.
  Fetching,
  // Indicates the module dependencies are being fetched.
  Resolving,
  // Indicates the module has ben seen before.
  Duplicate,
  // Indicates the module (include its dependencies) is resolved.
  Ready,
}

#[derive(Debug)]
/// ECMAScript module, i.e. the `import` module.
pub struct EsModule {
  /// Module path on local file system.
  pub path: ModulePath,
  /// Module import status.
  pub status: ModuleStatus,
  /// Maps the module itself to all its dependencies.
  pub dependencies: Vec<Rc<RefCell<EsModule>>>,
  /// Exceptions when import.
  pub exception: Rc<RefCell<Option<String>>>,
  /// Whether this module is dynamically import.
  pub is_dynamic_import: bool,
}

impl EsModule {
  // Traverses the dependency tree to check if the module is ready.
  pub fn fast_forward(&mut self, seen_modules: &mut HashMap<ModulePath, ModuleStatus>) {
    // If the module is ready, no need to check the sub-tree.
    if self.status == ModuleStatus::Ready {
      return;
    }

    // If it's a duplicate module we need to check the module status cache.
    if self.status == ModuleStatus::Duplicate {
      let status_ref = seen_modules.get(&self.path).unwrap();
      if status_ref == &ModuleStatus::Ready {
        self.status = ModuleStatus::Ready;
      }
      return;
    }

    // Fast-forward all dependencies.
    self
      .dependencies
      .iter_mut()
      .for_each(|dep| dep.borrow_mut().fast_forward(seen_modules));

    // The module is compiled and has 0 dependencies.
    if self.dependencies.is_empty() && self.status == ModuleStatus::Resolving {
      self.status = ModuleStatus::Ready;
      seen_modules.insert(self.path.clone(), self.status);
      return;
    }

    // At this point, the module is still being fetched...
    if self.dependencies.is_empty() {
      return;
    }

    if !self
      .dependencies
      .iter_mut()
      .map(|m| m.borrow().status)
      .any(|status| status != ModuleStatus::Ready)
    {
      self.status = ModuleStatus::Ready;
      seen_modules.insert(self.path.clone(), self.status);
    }
  }
}

#[derive(Debug)]
/// Module graph.
pub struct ModuleGraph {
  pub kind: ImportKind,
  pub root_rc: Rc<RefCell<EsModule>>,
  pub same_origin: LinkedList<v8::Global<v8::PromiseResolver>>,
}

impl ModuleGraph {
  // Initializes a new graph resolving a static import.
  pub fn static_import(path: &str) -> ModuleGraph {
    // Create an ES module instance.
    let module = Rc::new(RefCell::new(EsModule {
      path: path.into(),
      status: ModuleStatus::Fetching,
      dependencies: vec![],
      exception: Rc::new(RefCell::new(None)),
      is_dynamic_import: false,
    }));

    Self {
      kind: ImportKind::Static,
      root_rc: module,
      same_origin: LinkedList::new(),
    }
  }

  // Initializes a new graph resolving a dynamic import.
  pub fn dynamic_import(path: &str, promise: v8::Global<v8::PromiseResolver>) -> ModuleGraph {
    // Create an ES module instance.
    let module = Rc::new(RefCell::new(EsModule {
      path: path.into(),
      status: ModuleStatus::Fetching,
      dependencies: vec![],
      exception: Rc::new(RefCell::new(None)),
      is_dynamic_import: true,
    }));

    Self {
      kind: ImportKind::Dynamic(promise),
      root_rc: module,
      same_origin: LinkedList::new(),
    }
  }
}

/// Module map.
/// It maintains all the modules inside js runtime, including already resolved and pending
/// fetching.
pub struct ModuleMap {
  pub main: Option<ModulePath>,
  pub index: HashMap<ModulePath, v8::Global<v8::Module>>,
  pub seen: HashMap<ModulePath, ModuleStatus>,
  pub pending: Vec<Rc<RefCell<ModuleGraph>>>,
}

impl ModuleMap {
  // Creates a new module-map instance.
  pub fn new() -> ModuleMap {
    Self {
      main: None,
      index: HashMap::new(),
      seen: HashMap::new(),
      pending: vec![],
    }
  }

  // Inserts a compiled ES module to the map.
  pub fn insert(&mut self, path: &str, module: v8::Global<v8::Module>) {
    // No main module has been set, so let's update the value.
    if self.main.is_none() && std::fs::metadata(path).is_ok() {
      self.main = Some(path.into());
    }
    self.index.insert(path.into(), module);
  }

  // Returns if there are still pending imports to be loaded.
  pub fn has_pending_imports(&self) -> bool {
    !self.pending.is_empty()
  }

  // Returns a v8 module reference from me module-map.
  pub fn get(&self, key: &str) -> Option<v8::Global<v8::Module>> {
    self.index.get(key).cloned()
  }

  // Returns a specifier by a v8 module.
  pub fn get_path(&self, module: v8::Global<v8::Module>) -> Option<ModulePath> {
    self
      .index
      .iter()
      .find(|(_, m)| **m == module)
      .map(|(p, _)| p.clone())
  }

  // Returns the main entry point.
  pub fn main(&self) -> Option<ModulePath> {
    self.main.clone()
  }
}

impl Default for ModuleMap {
  fn default() -> Self {
    ModuleMap::new()
  }
}

/// A single import mapping (specifier, target).
type ImportMapEntry = (String, String);

/// Key-Value entries representing WICG import-maps.
/// See: <https://github.com/WICG/import-maps>.
///
/// NOTE: This is just a mock-up which is actually not supported.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ImportMap {
  map: Vec<ImportMapEntry>,
}

impl ImportMap {
  pub fn parse_from_json(_text: &str) -> AnyResult<ImportMap> {
    Ok(ImportMap { map: Vec::new() })
  }

  pub fn lookup(&self, _specifier: &str) -> Option<String> {
    None
  }

  // /// Creates an ImportMap from JSON text.
  // pub fn parse_from_json(text: &str) -> AnyResult<ImportMap> {
  //   // Parse JSON string into serde value.
  //   let json: serde_json::Value = serde_json::from_str(text)?;
  //   let imports = json["imports"].to_owned();
  //
  //   if imports.is_null() || !imports.is_object() {
  //     return Err(anyhow::anyhow!("Import map's 'imports' must be an object"));
  //   }
  //
  //   let map: HashMap<String, String> = serde_json::from_value(imports)?;
  //   let mut map: Vec<ImportMapEntry> = Vec::from_iter(map);
  //
  //   // Note: We're sorting the imports because we need to support "Packages"
  //   // via trailing slashes, so the lengthier mapping should always be selected.
  //   //
  //   // https://github.com/WICG/import-maps#packages-via-trailing-slashes
  //
  //   map.sort_by(|a, b| b.0.cmp(&a.0));
  //
  //   Ok(ImportMap { map })
  // }
  //
  // /// Tries to match a specifier against an import-map entry.
  // pub fn lookup(&self, specifier: &str) -> Option<String> {
  //   // Find a mapping if exists.
  //   let (base, mut target) = match self.map.iter().find(|(k, _)| specifier.starts_with(k)) {
  //     Some(mapping) => mapping.to_owned(),
  //     None => return None,
  //   };
  //
  //   // The following code treats "./" as an alias for the CWD.
  //   if target.starts_with("./") {
  //     let cwd = env::current_dir().unwrap().to_string_lossy().to_string();
  //     target = target.replacen('.', &cwd, 1);
  //   }
  //
  //   // Note: The reason we need this additional check below with the specifier's
  //   // extension (if exists) is to be able to support extension-less imports.
  //   //
  //   // https://github.com/WICG/import-maps#extension-less-imports
  //
  //   match Path::new(specifier).extension() {
  //     Some(ext) => match Path::new(specifier) == Path::new(&base).with_extension(ext) {
  //       false => Some(specifier.replacen(&base, &target, 1)),
  //       _ => None,
  //     },
  //     None => Some(specifier.replacen(&base, &target, 1)),
  //   }
  // }
}

/// Resolves an import using the appropriate loader.
/// Returns full path on local file system.
pub fn resolve_import(
  base: Option<&str>,
  specifier: &str,
  ignore_core_modules: bool,
  import_map: Option<ImportMap>,
) -> AnyResult<ModulePath> {
  // Use import-maps if available.
  let specifier = match import_map {
    Some(map) => map.lookup(specifier).unwrap_or_else(|| specifier.into()),
    None => specifier.into(),
  };

  // Look the params and choose a loader, then resolve module.
  let is_core_module_import = CORE_MODULES().contains_key(specifier.as_str());
  if is_core_module_import && !ignore_core_modules {
    CoreModuleLoader {}.resolve(base, &specifier)
  } else {
    FsModuleLoader {}.resolve(base, &specifier)
  }
}

/// Loads an import using the appropriate loader.
pub fn load_import(specifier: &str, _skip_cache: bool) -> AnyResult<ModuleSource> {
  // // Look the params and choose a loader.
  // let loader: Box<dyn ModuleLoader> = match (
  //   CORE_MODULES().contains_key(specifier),
  //   WINDOWS_REGEX().is_match(specifier),
  //   Url::parse(specifier).is_ok(),
  // ) {
  //   (true, _, _) => Box::new(CoreModuleLoader),
  //   (_, true, _) => Box::new(FsModuleLoader),
  //   (_, _, true) => Box::new(UrlModuleLoader { skip_cache }),
  //   _ => Box::new(FsModuleLoader),
  // };
  //
  // // Load module.
  // loader.load(specifier)

  let is_core_module_import = CORE_MODULES().contains_key(specifier);
  if is_core_module_import {
    CoreModuleLoader {}.load(specifier)
  } else {
    FsModuleLoader {}.load(specifier)
  }

  // // We don't actually have core modules
  // FsModuleLoader {}.load(specifier)
}

pub async fn load_import_async(specifier: &str, skip_cache: bool) -> AnyResult<ModuleSource> {
  load_import(specifier, skip_cache)
}

/// Resolves module imports synchronously.
/// See: <https://source.chromium.org/chromium/v8/v8.git/+/51e736ca62bd5c7bfd82488a5587fed31dbf45d5:src/d8.cc;l=741>.
pub fn fetch_module_tree<'a>(
  scope: &mut v8::HandleScope<'a>,
  filename: &str,
  source: Option<&str>,
) -> Option<v8::Local<'a, v8::Module>> {
  // Create a script origin.
  let origin = create_origin(scope, filename, true);
  let state = JsRuntime::state(scope);

  // Find appropriate loader if source is empty.
  let source = match source {
    Some(source) => source.into(),
    None => load_import(filename, true).unwrap(),
  };
  trace!(
    "Loaded main js module filename: {:?}, source: {:?}",
    filename,
    if source.as_str().len() > 20 {
      String::from(&source.as_str()[..20]) + "..."
    } else {
      String::from(source.as_str())
    }
  );
  let source = v8::String::new(scope, &source).unwrap();
  let mut source = v8::script_compiler::Source::new(source, Some(&origin));

  #[allow(clippy::question_mark)]
  let module = match v8::script_compiler::compile_module(scope, &mut source) {
    Some(module) => module,
    None => {
      // Early return.
      return None;
    }
  };

  // Subscribe module to the module-map.
  let module_ref = v8::Global::new(scope, module);
  state.borrow_mut().module_map.insert(filename, module_ref);

  let requests = module.get_module_requests();
  trace!("Get {} module requests", requests.length());

  for i in 0..requests.length() {
    // Get import request from the `module_requests` array.
    let request = requests.get(scope, i).unwrap();
    let request = v8::Local::<v8::ModuleRequest>::try_from(request).unwrap();

    // Transform v8's ModuleRequest into Rust string.
    let specifier = request.get_specifier().to_rust_string_lossy(scope);
    let specifier = resolve_import(Some(filename), &specifier, false, None).unwrap();
    trace!(
      "Resolved dependency js module base: {:?}, specifier: {:?}",
      filename,
      specifier.as_str(),
    );

    // Resolve subtree of modules.
    if !state.borrow().module_map.index.contains_key(&specifier) {
      fetch_module_tree(scope, &specifier, None)?;
    }
  }

  Some(module)
}
