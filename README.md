````rust
use std::collections::HashMap;

use simple_ssr_rs::{SSRender,ssr_work,Value};

use simple_ssr_rs::salvo::{self,prelude::*};

// Extend router
struct Hello(TeraBuilder);
#[handler]
impl Hello{
  async fn handle(&self, req: &mut Request, _depot: &mut Depot, res: &mut Response){
      //let mut ctx = Context::default();
      // let ctx = self.0.gen_context(req);
      // match self.0.build(ctx.clone()){
      //   Ok((tera,ctx))=>{
      //     let r = tera.render("index.html", &ctx).unwrap_or("".to_owned());
      //     let _ = res.add_header("server", "xfinal", true);
      //     res.render(Text::Html(r));
      //   }
      //   Err(_)=>{
      //     res.render(Text::Plain("Error"));
      //   }
      // }
      res.render(Text::Plain("Hello"));
  }
}

#[handler]
async fn handle404(&self, _req: &Request, _depot: &Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    if let Some(StatusCode::NOT_FOUND) = res.status_code {
        res.render("Custom 404 Error Page");
        ctrl.skip_rest();
    }
}

fn main() {
   let mut ssr = SSRender::new("0.0.0.0:8080");
   ssr.set_pub_dir_name("assets");  // specify the name of the public assets directory in the current root directory
   ssr.set_tmpl_dir_name("pages");  // specify the name of the template directory in the current root directory
  // ssr.set_meta_info_collector(|req:&Request|->HashMap<String, Value>{}); // get meta data from the current request for the tempalte
   ssr.register_function("println".to_owned(), |v:&HashMap<String, Value>|{  // register a function that can be used in template files
	  let r = v.get("value").ok_or("none")?.as_str().ok_or("none")?;
	  Ok(Value::String(format!("<p>{r}</p><br/>")))
   });
   ssr.set_ctx_generator(|_req:&Request|->HashMap<String, Value>{  // collect infomation from the current request, these objects can be used in the templates
      let mut map = HashMap::new();
      map.insert("info".to_owned(), Value::Bool(true));
      map
   });
   // ssr.set_hook_view_path(Some(|req,path|{ path})); // modify the path before rendering it by view-engine
   //let router = Router::with_path("base");  // root path
   //let router = router.push(Router::with_path("hello").get(Hello(ssr.gen_tera_builder())));
   // ssr_work!(ssr,router);
   // ssr_work!(ssr,None,Catcher::default().hoop(handle404));
   // ssr_work!(ssr,router,Catcher::default().hoop(handle404));
   ssr_work!(ssr);
}

````

#### Use built-in function
````html
<!-- 
	/pages/common/abc.html
	<h3>{{context.title}}</h3>
   <div>{{parent.info}}</div>  we can access the variable in the parent scope by using __Parent.info(if any), and so forth, __Parent...__Parent.info 
-->
<!-- the common directory is in the root path `pages`, the following content is in the index.html -->
<div>
	{{ include_file(path="common/abc.html"), context=`{"title":"abc"}` | safe }}
</div>
````

Then we can access the website  on the browser: http://localhost:8080/ or http://localhost:8080/index.html. Access the static resource on browser: http://localhost:8080/assets/filename

More details about how to use the template engine can be seen on the home page of [Tera](https://tera.netlify.app/docs/).

