pub use anyhow;
pub use salvo;
pub use salvo::catcher::Catcher;
#[cfg(feature = "http3")]
pub use salvo::conn::rustls::{Keycert, RustlsConfig};
#[cfg(feature = "http3")]
use salvo::conn::tcp::TcpAcceptor;

use salvo::prelude::*;
use salvo::serve_static::StaticDir;
pub use serde_json::{self, Value};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
pub use tera::{self, Context, Filter, Function, Tera};
pub use tokio::{self};

type TeraFunctionMap = HashMap<String, Arc<dyn Function + 'static>>;
type TeraFilterMap = HashMap<String, Arc<dyn Filter + 'static>>;
type MetaInfoCollector =
    Option<Arc<dyn Fn(&Request) -> HashMap<String, Value> + 'static + Send + Sync>>;
struct CallableObjectForTera<F: ?Sized>(Arc<F>);

impl<F: Function + ?Sized> Function for CallableObjectForTera<F> {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        self.0.call(args)
    }
}

impl<F: Filter + ?Sized> Filter for CallableObjectForTera<F> {
    fn filter(&self, value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
        self.0.filter(value, args)
    }
}

pub struct Http3Certification {
    pub cert: std::path::PathBuf,
    pub key: std::path::PathBuf,
}

pub struct SSRender<ErrorWriter: Writer + From<anyhow::Error> + From<tera::Error> = anyhow::Error> {
    pub_assets_dir_name: String,
    tmpl_dir_name: String,
    host: String,
    tmpl_func_map: TeraFunctionMap,
    tmpl_filter_map: TeraFilterMap,
    ctx_generator: MetaInfoCollector,
    phantom_data_: PhantomData<ErrorWriter>,
    default_view_file_postfix: String,
    default_view_file_name: String,
    listing_assets: bool,
    default_asset_filename: Option<String>,
    #[cfg(feature = "http3")]
    use_http3: Option<Http3Certification>,
}
impl<ErrorWriter: Writer + From<anyhow::Error> + From<tera::Error> + Send + Sync + 'static>
    SSRender<ErrorWriter>
{
    pub fn new(host: &str) -> Self {
        Self {
            pub_assets_dir_name: "public".to_owned(),
            tmpl_dir_name: "templates".to_owned(),
            host: host.to_owned(),
            tmpl_func_map: HashMap::new(),
            tmpl_filter_map: HashMap::new(),
            ctx_generator: None,
            phantom_data_: PhantomData,
            default_view_file_postfix: "html".to_owned(),
            default_view_file_name: "index.html".to_owned(),
            listing_assets: true,
            default_asset_filename: None,
            #[cfg(feature = "http3")]
            use_http3: None,
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

    pub fn set_default_file_postfix(&mut self, postfix: &str) {
        self.default_view_file_postfix = postfix.to_owned();
    }

    pub fn default_file_postfix(&self) -> &str {
        &self.default_view_file_postfix
    }

    pub fn set_listing_assets(&mut self, v: bool) {
        self.listing_assets = v;
    }

    pub fn listing_assets(&self) -> bool {
        self.listing_assets
    }

    pub fn set_default_assets_filename(&mut self, v: &str) {
        self.default_asset_filename = Some(v.to_owned());
    }

    pub fn default_assets_filename(&self) -> &Option<String> {
        &self.default_asset_filename
    }
    #[cfg(feature = "http3")]
    pub fn set_use_http3(&mut self, cert: Http3Certification) {
        self.use_http3 = Some(cert);
    }
    #[cfg(feature = "http3")]
    pub fn use_http3(&self) -> Option<&Http3Certification> {
        self.use_http3.as_ref()
    }

    pub async fn serve(&self, extend_router: Option<Router>, catcher: Option<Catcher>) {
        let pub_assets_router = Router::with_path(format!("{}/<**>", self.pub_assets_dir_name))
            .get(
                StaticDir::new([&self.pub_assets_dir_name])
                    .defaults(match &self.default_asset_filename {
                        Some(v) => {
                            vec![v.to_owned()]
                        }
                        None => {
                            vec![]
                        }
                    })
                    .listing(self.listing_assets),
            );
        let view_router = Router::with_path("/<**rest_path>").get(ViewHandler::<ErrorWriter>::new(
            self.gen_tera_builder(),
            self.default_view_file_postfix.clone(),
            self.default_view_file_name.clone(),
        ));
        //let router = Router::new();

        let router = match extend_router {
            Some(r) => r,
            None => Router::new(),
        };
        let router = router.push(pub_assets_router);
        let router = router.push(view_router);
        #[cfg(feature = "http3")]
        enum VariantAcceptor<U> {
            NonHttp3(TcpAcceptor),
            Http3(U),
        }

        #[cfg(feature = "http3")]
        let var_acceptor = match self.use_http3.as_ref() {
            Some(cert) => {
                let cert_bytes = tokio::fs::read(&cert.cert).await.unwrap();
                let key_bytes = tokio::fs::read(&cert.key).await.unwrap();
                let config = RustlsConfig::new(
                    Keycert::new()
                        .cert(cert_bytes.as_slice())
                        .key(key_bytes.as_slice()),
                );
                let listener = TcpListener::new(self.host.clone()).rustls(config.clone());
                let acceptor = QuinnListener::new(config, ("127.0.0.1", 5800))
                    .join(listener)
                    .bind()
                    .await;
                VariantAcceptor::Http3(acceptor)
            }
            None => {
                let acceptor = TcpListener::new(&self.host).bind().await;
                VariantAcceptor::NonHttp3(acceptor)
            }
        };

        match catcher {
            Some(catcher) => {
                let service = Service::new(router).catcher(catcher);
                #[cfg(feature = "http3")]
                {
                    match var_acceptor {
                        VariantAcceptor::Http3(acceptor) => {
                            Server::new(acceptor).serve(service).await;
                        }
                        VariantAcceptor::NonHttp3(acceptor) => {
                            Server::new(acceptor).serve(service).await;
                        }
                    }
                }
                #[cfg(not(feature = "http3"))]
                {
                    let acceptor = TcpListener::new(&self.host).bind().await;
                    Server::new(acceptor).serve(service).await;
                }
            }
            None => {
                #[cfg(feature = "http3")]
                {
                    match var_acceptor {
                        VariantAcceptor::Http3(acceptor) => {
                            Server::new(acceptor).serve(router).await;
                        }
                        VariantAcceptor::NonHttp3(acceptor) => {
                            Server::new(acceptor).serve(router).await;
                        }
                    }
                }
                #[cfg(not(feature = "http3"))]
                {
                    let acceptor = TcpListener::new(&self.host).bind().await;
                    Server::new(acceptor).serve(router).await;
                }
            }
        };
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

    pub fn build(&self, ctx: Context) -> tera::Result<(Tera, Context)> {
        let mut tera = Tera::new(&self.tpl_dir)?;
        self.register_utilities(&mut tera);
        tera.register_filter(
            "json_decode",
            |v: &Value, _args: &HashMap<String, Value>| -> tera::Result<Value> {
                let v = v
                    .as_str()
                    .ok_or(tera::Error::msg("value must be a json object string"))?;
                let v = serde_json::from_str::<Value>(v)?;
                Ok(v)
            },
        );
        tera.register_function("include_file", generate_include(tera.clone(), ctx.clone()));
        Ok((tera, ctx))
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

struct ViewHandler<ErrorWriter: Writer + From<anyhow::Error> + From<tera::Error> = anyhow::Error> {
    tera_builder: TeraBuilder,
    phantom_data_: PhantomData<ErrorWriter>,
    default_postfix: String,
    default_view_file_name: String,
}
impl<ErrorWriter: Writer + From<anyhow::Error> + From<tera::Error>> ViewHandler<ErrorWriter> {
    fn new(
        tera_builder: TeraBuilder,
        default_postfix: String,
        default_view_file_name: String,
    ) -> Self {
        Self {
            tera_builder,
            phantom_data_: PhantomData,
            default_postfix,
            default_view_file_name,
        }
    }
}
#[handler]
impl<ErrorWriter: Writer + From<anyhow::Error> + From<tera::Error> + Send + Sync + 'static>
    ViewHandler<ErrorWriter>
{
    async fn handle(
        &self,
        req: &mut Request,
        _depot: &mut Depot,
        res: &mut Response,
    ) -> Result<(), ErrorWriter> {
        let Some(path) = req.param::<String>("**rest_path") else{
			res.status_code(StatusCode::BAD_REQUEST);
			return Err(anyhow::format_err!("invalid request path").into());
		};
        let ctx = self.tera_builder.gen_context(req);
        let path = if path.is_empty() {
            format!("{}", self.default_view_file_name)
        } else {
            match path.rfind(".") {
                Some(_) => path,
                None => {
                    format!("{path}.{}", self.default_postfix)
                }
            }
        };
        if !cfg!(debug_assertions) {
            let (tera, ctx) = self.tera_builder.build(ctx.clone())?;
            match tera.render(&path, &ctx) {
                Ok(html) => {
                    res.render(Text::Html(html));
                }
                Err(e) => {
                    if let tera::ErrorKind::TemplateNotFound(_) = &e.kind {
                        res.status_code(StatusCode::NOT_FOUND);
                    } else {
                        res.status_code(StatusCode::BAD_REQUEST);
                    }
                    return Err(anyhow::format_err!("{}", e.to_string()).into());
                }
            };
        } else {
            match self.tera_builder.build(ctx.clone()) {
                Ok((tera, ctx)) => match tera.render(&path, &ctx) {
                    Ok(s) => {
                        res.render(Text::Html(s));
                    }
                    Err(e) => {
                        if let tera::ErrorKind::TemplateNotFound(_) = &e.kind {
                            res.status_code(StatusCode::NOT_FOUND);
                        } else {
                            res.status_code(StatusCode::BAD_REQUEST);
                        }
                        return Err(anyhow::format_err!("{e:?}").into());
                    }
                },
                Err(e) => {
                    res.status_code(StatusCode::BAD_REQUEST);
                    return Err(anyhow::format_err!("{e:?}").into());
                }
            };
        }
        Ok(())
    }
}

fn generate_include(tera: Tera, parent: Context) -> impl Function {
    move |args: &HashMap<String, Value>| -> tera::Result<Value> {
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
    ($e:expr, None, $catcher:expr) => {
        $crate::tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                $e.serve(None, Some($catcher)).await;
            });
    };
    ($e:expr, $router:expr, $catcher:expr) => {
        $crate::tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                $e.serve(Some($router), Some($catcher)).await;
            });
    };
    ($e:expr, $router:expr) => {
        $crate::tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                $e.serve(Some($router), None).await;
            });
    };
    ($e:expr) => {
        $crate::tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                $e.serve(None, None).await;
            });
    };
}
