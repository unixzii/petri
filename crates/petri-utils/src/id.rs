use std::borrow::Borrow;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Id(Arc<str>);

impl Id {
    pub fn new(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for Id {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<&String> for Id {
    fn from(value: &String) -> Self {
        Self::new(value.as_str())
    }
}

impl From<String> for Id {
    fn from(value: String) -> Self {
        Self::new(value.as_str())
    }
}

impl ToString for Id {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Borrow<str> for Id {
    fn borrow(&self) -> &str {
        &*self.0
    }
}

impl Deref for Id {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
