use crate::models::{User, UserRole};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};
use uuid::Uuid;

pub struct AuthMiddleware {
    required_role: Option<UserRole>,
}

impl AuthMiddleware {
    pub fn new(role: Option<UserRole>) -> Self {
        Self { required_role: role }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service,
            required_role: self.required_role.clone(),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
    required_role: Option<UserRole>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let user_header = req.headers().get("X-User-ID");
        let role_header = req.headers().get("X-User-Role");

        let user = match (user_header, role_header) {
            (Some(id), Some(role)) => {
                let role_str = role.to_str().unwrap_or("");
                let role = match role_str {
                    "super_admin" => UserRole::SuperAdmin,
                    "node_admin" => UserRole::NodeAdmin,
                    "business_line_admin" => UserRole::BusinessLineAdmin,
                    _ => {
                        return Box::pin(async move {
                            Err(actix_web::error::ErrorUnauthorized("Invalid role"))
                        });
                    }
                };

                if let Some(required) = &self.required_role {
                    let has_access = match (required, &role) {
                        (UserRole::SuperAdmin, UserRole::SuperAdmin) => true,
                        (UserRole::NodeAdmin, UserRole::SuperAdmin) => true,
                        (UserRole::NodeAdmin, UserRole::NodeAdmin) => true,
                        (UserRole::BusinessLineAdmin, UserRole::SuperAdmin) => true,
                        (UserRole::BusinessLineAdmin, UserRole::NodeAdmin) => true,
                        (UserRole::BusinessLineAdmin, UserRole::BusinessLineAdmin) => true,
                        _ => false,
                    };
                    if !has_access {
                        return Box::pin(async move {
                            Err(actix_web::error::ErrorForbidden("Insufficient permissions"))
                        });
                    }
                }

                Some(User {
                    id: Uuid::parse_str(id.to_str().unwrap_or("")).unwrap_or_else(|_| Uuid::nil()),
                    name: req
                        .headers()
                        .get("X-User-Name")
                        .and_then(|h| h.to_str().ok())
                        .unwrap_or("anonymous")
                        .to_string(),
                    role,
                    managed_nodes: vec![],
                    managed_business_lines: vec![],
                })
            }
            _ => None,
        };

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}

pub fn get_user_from_headers(req: &ServiceRequest) -> Option<User> {
    let id = req.headers().get("X-User-ID")?.to_str().ok()?;
    let role_str = req.headers().get("X-User-Role")?.to_str().ok()?;
    let name = req
        .headers()
        .get("X-User-Name")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();

    let role = match role_str {
        "super_admin" => UserRole::SuperAdmin,
        "node_admin" => UserRole::NodeAdmin,
        "business_line_admin" => UserRole::BusinessLineAdmin,
        _ => return None,
    };

    Some(User {
        id: Uuid::parse_str(id).unwrap_or_else(|_| Uuid::nil()),
        name,
        role,
        managed_nodes: vec![],
        managed_business_lines: vec![],
    })
}
