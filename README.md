````rust
use std::collections::HashMap;

use simple_ssr_rs::{SSRender,ssr_work,Value};
fn main() {
   let mut ssr = SSRender::new("0.0.0.0:8080");
   ssr.set_pub_dir_name("assets");  // specify the name of the public assets directory in the current root directory
   ssr.set_tmpl_dir_name("pages");  // specify the name of the template directory in the current root directory
  // ssr.set_meta_info_collector(|req:&Request|->HashMap<String, Value>{}); // get meta data from the current request for the tempalte
   ssr.register_function("println".to_owned(), |v:&HashMap<String, Value>|{  // register a function that can be used in template files
	  let r = v.get("value").ok_or("none")?.as_str().ok_or("none")?;
	  Ok(Value::String(format!("<p>{r}</p><br/>")))
   });
   ssr_work!(ssr);
}

````

#### Use built-in function
````html
<!-- 
	abc.html in the common directory
	<h3>{{context.title}}</h3>
-->
<!-- the common directory is in the same level path with the following HTML file -->
<div>
	{{ include_file(path="common/abc.html"), context=`{"title":"abc"}` | safe }}
</div>
````

More details about how to use the template engine can be seen on the home page of [Tera](https://tera.netlify.app/docs/).

