pub struct HasDrop;

pub trait I32Method {
    fn get_i32(&self) -> i32;
}

impl I32Method for HasDrop {
    fn get_i32(&self) -> i32 {
        3
    }
}

impl Drop for HasDrop {
    fn drop(&mut self) {
        println!("Dropping HasDrop");
    }
}

pub fn get_i32_method() -> Box<dyn I32Method> {
    Box::new(HasDrop) as _
}
