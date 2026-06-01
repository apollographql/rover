use url::Url;

#[cfg_attr(any(test, feature = "testing"), mockall::automock(type Error = std::io::Error;))]
pub trait OpenUrl {
    type Error: std::error::Error;
    fn open_url(&self, url: &Url) -> Result<(), Self::Error>;
}

#[derive(Default, Clone, Debug)]
pub struct SystemOpenUrl {}

impl OpenUrl for SystemOpenUrl {
    type Error = std::io::Error;
    fn open_url(&self, url: &Url) -> Result<(), Self::Error> {
        webbrowser::open(url.as_str())
    }
}

#[derive(Default, Debug)]
pub struct NoopOpenUrl {}

impl OpenUrl for NoopOpenUrl {
    type Error = std::io::Error;
    fn open_url(&self, _: &Url) -> Result<(), Self::Error> {
        Ok(())
    }
}
