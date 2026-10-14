use flate2::read::GzDecoder;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use reqwest::{Client, Error};
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use tar::Archive;

#[derive(Deserialize, Debug)]
struct IndexMenifestPlatform {
    architecture: String,
    os: String,
}

#[derive(Deserialize, Debug)]
struct IndexMenifest {
    platform: IndexMenifestPlatform,
    digest: String,
    variant: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DockerIndexResponse {
    manifests: Vec<IndexMenifest>,
}

#[derive(Deserialize, Debug)]
struct LayerInfo {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    size: u32,
}

#[derive(Deserialize, Debug)]
struct DockerImageLayersDataResponse {
    layers: Vec<LayerInfo>,
    config: LayerInfo,
}

pub struct ContainerPull {
    cid: String,
    name: String,
}

impl ContainerPull {
    pub fn new(cid: String, name: String) -> Self {
        Self { cid, name }
    }

    async fn get_token(&self) -> Result<Option<String>, Error> {
        let url = format!(
            "https://auth.docker.io/token?service=registry.docker.io&scope=repository:library/{0}:pull",
            self.name
        );
        let scope = format!("repository:library/{0}:pull", self.name);
        let client = Client::new();

        let response: serde_json::Value = client
            .get(url)
            .header("service", "registry.docker.io".to_string())
            .header("scope", scope)
            .send()
            .await?
            .json()
            .await?;

        let token = response
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(token)
    }

    async fn get_index(&self, token: &String) -> Result<Option<IndexMenifest>, Error> {
        let url = format!(
            "https://registry-1.docker.io/v2/library/{0}/manifests/latest",
            self.name
        );
        let client = Client::new();

        let response:DockerIndexResponse  = client.get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.index.v1+json, application/vnd.oci.image.manifest.v1+json")
        .send().await?
        .json()
        .await?;

        let manifest = response.manifests.into_iter().find(|index_menifest| {
            index_menifest.platform.os.eq("linux")
                && index_menifest.platform.architecture.eq("arm64")
                && index_menifest
                    .variant
                    .as_ref()
                    .is_none_or(|variant| variant.eq("v8"))
        });

        Ok(manifest)
    }

    async fn download_layer(
        &self,
        token: &String,
        layer: &LayerInfo,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Downloading {}", layer.digest);

        let url = format!(
            "https://registry-1.docker.io/v2/library/{0}/blobs/{1}",
            self.name, layer.digest
        );

        let client = Client::new();
        let bytes=  client.get(url)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header(ACCEPT, "application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.index.v1+json, application/vnd.oci.image.manifest.v1+json")
                .send().await?
                .bytes()
                .await?;

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    async fn download_layers(
        &self,
        token: &String,
        index: &IndexMenifest,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();
        let url = format!(
            "https://registry-1.docker.io/v2/library/{0}/manifests/{1}",
            self.name, index.digest
        );

        let response:DockerImageLayersDataResponse  = client.get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.index.v1+json, application/vnd.oci.image.manifest.v1+json")
        .send().await?
        .json()
        .await?;

        for layer in response.layers.iter() {
            let digest_part = match layer.digest.split(':').nth(1) {
                Some(part) => part,
                None => {
                    println!("Invalid digest format: {}", layer.digest);
                    continue;
                }
            };

            let full_path = format!("/tmp/jailer/sha256/{}", digest_part);
            let path = Path::new(&full_path);
            if !path.exists() {
                self.download_layer(token, layer, path).await?;
                continue;
            }

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let file = OpenOptions::new().read(true).open(path)?;

            let decompressed = GzDecoder::new(file);
            let mut archive = Archive::new(decompressed);
            let image_fs = format!("/tmp/jailer/fs/{}", self.name);
            archive.unpack(image_fs)?;
        }

        Ok(())
    }

    async fn pull(&self) -> Result<(), Box<dyn std::error::Error>> {
        let token = self.get_token().await?.unwrap();

        let index = self.get_index(&token).await?.unwrap();
        self.download_layers(&token, &index).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    // Import names from the outer module
    use super::*;

    // The #[test] attribute marks this function as a test case
    #[tokio::test]
    async fn test_all() {
        let cp = ContainerPull::new("axy".to_string(), "nginx".to_string());
        cp.pull().await.unwrap();
    }
}
