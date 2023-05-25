pub use salvo;
use salvo::prelude::*;
use salvo::serve_static::StaticDir;
pub use serde_json::{self, Value};
use std::{collections::HashMap, sync::Arc};
pub use tera::{self, Context, Filter, Function, Result, Tera};
pub use tokio::{self};
pub use anyhow;

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
    ctx_generator: MetaInfoCollector,
}
impl SSRender {
    pub fn new(host: &str) -> Self {
        Self {
            pub_assets_dir_name: "public".to_owned(),
            tmpl_dir_name: "templates".to_owned(),
            host: host.to_owned(),
            tmpl_func_map: HashMap::new(),
            tmpl_filter_map: HashMap::new(),
            ctx_generator: None,
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

    pub fn registed_functions(&self) -> &TeraFunctionMap {
        &self.tmpl_func_map
    }

    pub fn register_filter<F: Filter + 'static>(&mut self, k: String, f: F) {
        self.tmpl_filter_map.insert(k, Arc::new(f));
    }

    pub fn rm_registed_filter(&mut self, k: String) {
        self.tmpl_filter_map.remove(&k);
    }

    pub fn registed_filters(&self) -> &TeraFilterMap {
        &self.tmpl_filter_map
    }

    pub fn pub_dir_name(&self) -> &str {
        &self.pub_assets_dir_name
    }

    pub fn tmpl_dir_name(&self) -> &str {
        &self.tmpl_dir_name
    }

    pub fn set_ctx_generator(
        &mut self,
        f: impl Fn(&Request) -> HashMap<String, Value> + 'static + Send + Sync,
    ) {
        self.ctx_generator = Some(Arc::new(f));
    }

    pub fn rm_ctx_generator(&mut self) {
        self.ctx_generator = None;
    }

    pub fn gen_tera_builder(&self) -> TeraBuilder {
        TeraBuilder::new(
            format!("{}/**/*", self.tmpl_dir_name),
            self.tmpl_func_map.clone(),
            self.tmpl_filter_map.clone(),
            self.ctx_generator.clone(),
        )
    }

    pub async fn serve(&self, extend_router: Option<Router>) {
        let pub_assets_router = Router::with_path(format!("{}/<**>", self.pub_assets_dir_name))
            .get(
                StaticDir::new([&self.pub_assets_dir_name])
                    .defaults("index.html")
                    .listing(true),
            );
        let view_router =
            Router::with_path("/<**rest_path>").get(ViewHandler::new(self.gen_tera_builder()));
        //let router = Router::new();

        let router = match extend_router {
            Some(r) => r,
            None => Router::new(),
        };
        let router = router.push(pub_assets_router);
        let router = router.push(view_router);
        let acceptor = TcpListener::new(&self.host).bind().await;
        Server::new(acceptor).serve(router).await
    }
}

pub struct TeraBuilder {
    tpl_dir: String,
    tpl_funcs: TeraFunctionMap,
    tpl_filters: TeraFilterMap,
    ctx_generator: MetaInfoCollector,
}
impl TeraBuilder {
    pub fn new(
        tpl_dir: String,
        tpl_funcs: TeraFunctionMap,
        tpl_filters: TeraFilterMap,
        ctx_generator: MetaInfoCollector,
    ) -> Self {
        Self {
            tpl_dir,
            tpl_funcs,
            tpl_filters,
            ctx_generator,
        }
    }

    fn register_utilities(&self, tera: &mut Tera) {
        for (k, v) in &self.tpl_funcs {
            tera.register_function(k, CallableObjectForTera(Arc::clone(v)));
        }
        for (k, v) in &self.tpl_filters {
            tera.register_filter(k, CallableObjectForTera(Arc::clone(v)));
        }
    }

    pub fn build(&self, ctx: Context) -> tera::Result<(Tera,Context)> {
        let mut tera = Tera::new(&self.tpl_dir)?;
        self.register_utilities(&mut tera);
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
        tera.register_function("include_file", generate_include(tera.clone(), ctx.clone()));
        Ok((tera,ctx))
    }

    pub fn gen_context(&self, req: &Request) -> Context {
        match self.ctx_generator {
            Some(ref collect) => {
                let mut context = Context::new();
                for (k, val) in collect(req) {
                    context.insert(k, &val);
                }
                context
            }
            None => Context::default(),
        }
    }
}

struct ViewHandler {
    tera_builder: TeraBuilder,
}
impl ViewHandler {
    fn new(tera_builder: TeraBuilder) -> Self {
        Self { tera_builder }
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
        let ctx = self.tera_builder.gen_context(req);
        match self.tera_builder.build(ctx.clone()) {
            Ok((tera,ctx)) => {
                match tera.render(if path.is_empty() { "index.html" } else { &path }, &ctx) {
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

fn generate_include(tera: Tera, parent: Context) -> impl Function {
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
                let mut context = Context::from_value(serde_json::json!({ "context": v }))?;
                let mut tera = tera.clone();
                context.insert("__Parent", &parent.clone().into_json());
                tera.register_function(
                    "include_file",
                    generate_include(tera.clone(), context.clone()),
                );
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
                let mut context =
                    Context::from_value(serde_json::json!({ "context": Value::Null }))?;
                let mut tera = tera.clone();
                context.insert("__Parent", &parent.clone().into_json());
                tera.register_function(
                    "include_file",
                    generate_include(tera.clone(), context.clone()),
                );
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
    ($e:expr, $router:expr) => {
        $crate::tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                $e.serve(Some($router)).await;
            });
    };
    ($e:expr)=>{
        $crate::tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            $e.serve(None).await;
        });
    }
}
