extern crate jni;

use jni::JNIEnv;
use jni::objects::{JString};

#[no_mangle]
pub extern "system" fn Java_me_waterga_chalkydri_Chalkydri_getCamera<'local>(mut env: JNIEnv<'local>, input: JString<'local>) -> JClass<'local> {
}
