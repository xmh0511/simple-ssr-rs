pub use salvo;
use salvo::prelude::*;
use salvo::serve_static::StaticDir;
pub use serde_json::{self,Value};
use std::{collections::HashMap, sync::Arc};
pub use tera::{self,Context, Filter, Function, Result, Tera};
pub use tokio::{self};

type TeraFunctionMap = HashMap<String, Arc<dyn Function + 'static>>;
type TeraFilterMap = HashMap<String, Arc<dyn Filter + 'static>>;
type MetaInfoCollector =
    Option<Arc<dyn Fn(&Request) -> HashMap<String, Value> + 'static + Send + Sync>>;
struct CallableObjectForTera<F: ?Sized>(Arc<F>);

impl<F: Function + ?Sized> Function for CallableObjectForTera<F> {
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value> {
        self.0.call(args)
    }
}

impl<F: Filter + ?Sized> Filter for CallableObjectForTera<F> {
    fn filter(&self, value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
        self.0.filter(value, args)
    }
}

pub struct SSRender {
    pub_assets_dir_name: String,
    tmpl_dir_name: String,
    host: String,
    tmpl_func_map: TeraFunctionMap,
    tmpl_filter_map: TeraFilterMap,
    request_meta_info_collector: MetaInfoCollector,
}
impl SSRender {
    pub fn new(host: &str) -> Self {
        Self {
            pub_assets_dir_name: "public".to_owned(),
            tmpl_dir_name: "templates".to_owned(),
            host: host.to_owned(),
            tmpl_func_map: HashMap::new(),
            tmpl_filter_map: HashMap::new(),
            request_meta_info_collector: None,
        }
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn set_pub_dir_name(&mut self, path: &str) {
        self.pub_assets_dir_name = path.to_owned();
    }

    pub fn set_tmpl_dir_name(&mut self, path: &str) {
        self.tmpl_dir_name = path.to_owned();
    }

    pub fn register_function<F: Function + 'static>(&mut self, k: String, f: F) {
        self.tmpl_func_map.insert(k, Arc::new(f));
    }

    pub fn rm_registed_function(&mut self, k: String) {
        self.tmpl_func_map.remove(&k);
    }

    pub fn registed_function(&self) -> &TeraFunctionMap {
        &self.tmpl_func_map
    }

    pub fn pub_dir_name(&self) -> &str {
        &self.pub_assets_dir_name
    }

    pub fn tmpl_dir_name(&self) -> &str {
        &self.tmpl_dir_name
    }

    pub fn set_meta_info_collector(
        &mut self,
        f: impl Fn(&Request) -> HashMap<String, Value> + 'static + Send + Sync,
    ) {
        self.request_meta_info_collector = Some(Arc::new(f));
    }

	pub fn rm_meta_info_collector(& mut self){
		self.request_meta_info_collector = None;
	}

    pub async fn serve(&self) {
        let pub_assets_router = Router::with_path(format!("{}/<**>", self.pub_assets_dir_name))
            .get(
                StaticDir::new([&self.pub_assets_dir_name])
                    .defaults("index.html")
                    .listing(true),
            );
        let view_router = Router::with_path("/<**rest_path>").get(ViewHandler::new(
            format!("{}/**/*", self.tmpl_dir_name),
            self.tmpl_func_map.clone(),
            self.tmpl_filter_map.clone(),
            self.request_meta_info_collector.clone(),
        ));
        let router = Router::new().push(pub_assets_router);
        let router = router.push(view_router);
        let acceptor = TcpListener::new(&self.host).bind().await;
        Server::new(acceptor)
            .serve(router)
            .await
    }
}

struct ViewHandler {
    dir_path: String,
    tmpl_func_map: TeraFunctionMap,
    tmpl_filter_map: TeraFilterMap,
    request_meta_info_collector: MetaInfoCollector,
}
impl ViewHandler {
    fn new(
        v: String,
        tmp_func: TeraFunctionMap,
        filter_map: TeraFilterMap,
        collector: MetaInfoCollector,
    ) -> Self {
        Self {
            dir_path: v,
            tmpl_func_map: tmp_func,
            tmpl_filter_map: filter_map,
            request_meta_info_collector: collector,
        }
    }
    fn register_with_tera(&self, tera: &mut Tera) {
        for (k, v) in &self.tmpl_func_map {
            tera.register_function(k, CallableObjectForTera(Arc::clone(v)));
        }
        for (k, v) in &self.tmpl_filter_map {
            tera.register_filter(k, CallableObjectForTera(Arc::clone(v)));
        }
    }
    fn gen_context(&self, req:&Request) -> Context {
        match self.request_meta_info_collector {
            Some(ref collect) => {
				let mut context = Context::new();
				for (k,val) in collect(req){
					context.insert(k, &val);
				}
				context
			}
            None => {
				Context::default()
			}
        }
    }
}
#[handler]
impl ViewHandler {
    async fn handle(&self, req: &mut Request, _depot: &mut Depot, res: &mut Response) {
        let Some(path) = req.param::<String>("**rest_path") else{
			res.status_code(StatusCode::BAD_REQUEST);
			res.render(Text::Plain("invalid request path"));
			return;
		};
        match Tera::new(&self.dir_path) {
            Ok(mut tera) => {
                tera.register_function("include_file", generate_include(tera.clone()));
                tera.register_filter(
                    "json_decode",
                    |v: &Value, _args: &HashMap<String, Value>| -> Result<Value> {
                        let v = v
                            .as_str()
                            .ok_or(tera::Error::msg("value must be a json object string"))?;
                        let v = serde_json::from_str::<Value>(v)?;
                        Ok(v)
                    },
                );
                self.register_with_tera(&mut tera);
				let context = self.gen_context(req);
                match tera.render(
                    if path.is_empty() { "index.html" } else { &path },
                    &context,
                ) {
                    Ok(s) => {
                        res.render(Text::Html(s));
                    }
                    Err(e) => {
                        res.status_code(StatusCode::BAD_REQUEST);
                        res.render(Text::Plain(format!("{e:?}")));
                    }
                }
            }
            Err(e) => {
                res.status_code(StatusCode::BAD_REQUEST);
                res.render(Text::Plain(format!("{e:?}")));
            }
        }
    }
}

fn generate_include(tera: Tera) -> impl Function {
    move |args: &HashMap<String, Value>| -> Result<Value> {
        let Some(file_path) = args.get("path") else{
            return Err(tera::Error::msg("file does not exist in the template path"));
        };
        match args.get("context") {
            Some(v) => {
                //println!("value === {v}");
                let context_value = v
                    .as_str()
                    .ok_or(tera::Error::msg("context must be a json object string"))?;
                let v = serde_json::from_str::<Value>(context_value)?;
                let context = Context::from_value(serde_json::json!({ "context": v }))?;
                let r = tera
                    .render(
                        file_path
                            .as_str()
                            .ok_or(tera::Error::msg("template render error"))?,
                        &context,
                    )?
                    .to_string();
                return Ok(Value::String(r));
            }
            None => {
                let context = Context::from_value(serde_json::json!({ "context": Value::Null }))?;
                let r = tera
                    .render(
                        file_path
                            .as_str()
                            .ok_or(tera::Error::msg("template render error"))?,
                        &context,
                    )?
                    .to_string();
                return Ok(Value::String(r));
            }
        }
    }
}

#[macro_export]
macro_rules! ssr_work {
    ($e:expr) => {
        $crate::tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                $e.serve().await;
            });
    };
}
