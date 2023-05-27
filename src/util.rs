use aliri_braid::braid;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shuttle_service::{Factory, ResourceBuilder, Type};

#[braid(serde)]
pub struct AccessToken;

#[braid(serde)]
pub struct ClientId;

#[braid(serde)]
pub struct ClientSecret;

#[braid(serde)]
pub struct UserId;

#[braid(serde)]
pub struct MastodonInstance;

#[braid(serde)]
pub struct OauthState;

#[derive(Clone, Deserialize, Serialize)]
pub struct ServiceInfo {
    pub name: String,
}

pub struct ShuttleServiceInfo;

#[async_trait]
impl ResourceBuilder<ServiceInfo> for ShuttleServiceInfo {
    const TYPE: Type = Type::Secrets;

    type Config = ();
    type Output = ServiceInfo;

    fn new() -> Self {
        Self
    }

    fn config(&self) -> &Self::Config {
        &()
    }

    async fn output(
        self,
        factory: &mut dyn Factory,
    ) -> Result<Self::Output, shuttle_service::Error> {
        Ok(ServiceInfo {
            name: factory.get_service_name().to_string(),
        })
    }

    async fn build(build_data: &Self::Output) -> Result<ServiceInfo, shuttle_service::Error> {
        Ok(build_data.clone())
    }
}
