use crate::constants::HEADER_ROUTER_ERROR;
use worker::{Response, Result};

pub fn router_error(message: &str, status: u16, code: &str) -> Result<Response> {
    let mut response = Response::error(message, status)?;
    response.headers_mut().set(HEADER_ROUTER_ERROR, code)?;
    Ok(response)
}
