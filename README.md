````rust
use std::collections::HashMap;

use simple_ssr_rs::{SSRender,ssr_work,Value};
fn main() {
   let mut ssr = SSRender::new("0.0.0.0:8080");
   ssr.set_pub_dir_name("assets");  // specify the name of the public assets directory in the current root directory
   ssr.set_tmpl_dir_name("pages");  // specify the name of the template directory in the current root directory
   ssr.register_function("println".to_owned(), |v:&HashMap<String, Value>|{  // register a function that can be used in template files
	  let r = v.get("value").ok_or("none")?.as_str().ok_or("none")?;
	  Ok(Value::String(format!("<p>{r}</p><br/>")))
   });
   ssr_work!(ssr);
}

````

