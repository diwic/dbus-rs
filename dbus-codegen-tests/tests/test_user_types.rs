mod user_types_dbus;

use dbus_crossroads::Crossroads;

use codegen_tests::user_type::MyType;
use user_types_dbus::*;

impl user_types_cr::ComExampleMyService1InterestingInterface for () {
    fn method1(&mut self, arg1: codegen_tests::user_type::MyType) -> Result<codegen_tests::user_type::MyType, dbus::MethodErr>{
        return Ok(arg1);
    }
    fn bar(&self) -> Result<codegen_tests::user_type::MyType, dbus::MethodErr> {
        return Ok(MyType::new());
    }
    fn set_bar(&self, _value: codegen_tests::user_type::MyType) -> Result<(), dbus::MethodErr> {
        return Ok(());
    }
}


#[test]
fn test_cr() {
    let mut cr = Crossroads::new();
    let token = user_types_cr::register_com_example_my_service1_interesting_interface::<()>(&mut cr);
}

#[test]
fn test_blocking() {

}