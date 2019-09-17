#[macro_use]
extern crate lazy_static;
extern crate micro_http;
extern crate threadpool;

use micro_http::{Body, HttpConnection, Request, Response, StatusCode, Version};
use threadpool::ThreadPool;

use std::collections::HashMap;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::result;

#[derive(Debug)]
pub enum Error {
    EndpointMismatch,
}

pub type Result<T> = result::Result<T, Error>;

pub trait Route: Sync + Send {
    fn handle(&self, req: Request) -> Result<Response>;
}

struct VmCreate {}

impl Route for VmCreate {
    fn handle(&self, _req: Request) -> Result<Response> {
        let mut response = Response::new(Version::Http11, StatusCode::OK);
        response.set_body(Body::new("Vm Create DONE"));

        Ok(response)
    }
}

struct VmStart {}

impl Route for VmStart {
    fn handle(&self, _req: Request) -> Result<Response> {
        let mut response = Response::new(Version::Http11, StatusCode::OK);
        response.set_body(Body::new("Vm Start DONE"));

        Ok(response)
    }
}

struct ApiRoutes {
    routes: HashMap<String, Box<dyn Route + Sync + Send>>,
}

lazy_static! {
    static ref API_ROUTES: ApiRoutes = {
        let mut r = ApiRoutes {
            routes: HashMap::new(),
        };

        r.routes
            .insert("/vm.create".to_string(), Box::new(VmCreate {}));
        r.routes
            .insert("/vm.start".to_string(), Box::new(VmStart {}));
        r
    };
}

struct ApiServer<'a> {
    path: &'a Path,
    num_threads: usize,
}

impl<'a> ApiServer<'a> {
    fn new(path: &'a str, num_threads: usize) -> Self {
        ApiServer {
            path: Path::new(path),
            num_threads,
        }
    }

    fn start(&self) {
        let listener = UnixListener::bind(self.path).unwrap();
        let pool = ThreadPool::new(self.num_threads);

        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    pool.execute(move || {
                        let mut http_connection = HttpConnection::new(s);

                        http_connection.try_read().unwrap();
                        let request = http_connection.pop_parsed_request().unwrap();
                        let path = request.uri().get_abs_path().to_string();

                        let response = match API_ROUTES.routes.get(&path) {
                            Some(route) => {
                                println!("FOUND route for path {}", path);
                                route.handle(request).unwrap()
                            }
                            None => Response::new(Version::Http11, StatusCode::NotFound),
                        };

                        http_connection.enqueue_response(response);
                        http_connection.try_write().unwrap();
                    });
                }

                Err(_) => continue,
            }
        }

        pool.join();
    }
}

fn main() {
    ApiServer::new("/tmp/cloud-hypervisor.sock", 4).start();
}