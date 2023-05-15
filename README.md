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
   ssr.set_meta_info_collector(|_req:&Request|->HashMap<String, Value>{  // collect infomation from the current request, these objects can be used in the templates
      let mut map = HashMap::new();
      map.insert("info".to_owned(), Value::Bool(true));
      map
   });
   ssr_work!(ssr);
}

````

#### Use built-in function
````html
<!-- 
	/pages/common/abc.html
	<h3>{{context.title}}</h3>
   <div>{{parent.info}}</div>  we can access the variable in the parent(if any), and so forth, parent...parent.info 
-->
<!-- the common directory is in root path `pages` -->
<div>
	{{ include_file(path="common/abc.html"), context=`{"title":"abc"}` | safe }}
</div>
````

More details about how to use the template engine can be seen on the home page of [Tera](https://tera.netlify.app/docs/).

