use std::{fmt, str};

use super::{error::Error, header::Header};

const DELIM: &str = "\n---\n";

#[derive(Clone, Debug)]
pub struct PostSource {
    pub(super) header: Header,
    pub(super) markdown: String,
}

impl PostSource {
    #[inline]
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    #[inline]
    #[must_use]
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    #[inline]
    #[must_use]
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    #[inline]
    #[must_use]
    pub fn markdown_mut(&mut self) -> &mut String {
        &mut self.markdown
    }
}

impl str::FromStr for PostSource {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (header, markdown) = s.split_once(DELIM)
            .ok_or(Error::NoDelim)?;

        Ok(PostSource {
            header: header.parse()?,
            markdown: markdown.to_owned(),
        })
    }
}

impl fmt::Display for PostSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {  
        write!(f, "{}{}{}", self.header, DELIM, self.markdown)
    }
}
