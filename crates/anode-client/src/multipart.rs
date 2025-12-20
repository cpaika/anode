use crate::client::AnodeClient;
use crate::error::{ClientError, Result};
use bytes::Bytes;
use tracing::debug;

pub struct MultipartUpload<'a> {
    client: &'a AnodeClient,
    bucket: String,
    key: String,
    upload_id: Option<String>,
    parts: Vec<CompletedPart>,
    completed: bool,
}

impl<'a> MultipartUpload<'a> {
    pub(crate) fn new(client: &'a AnodeClient, bucket: String, key: String) -> Self {
        Self {
            client,
            bucket,
            key,
            upload_id: None,
            parts: Vec::new(),
            completed: false,
        }
    }

    pub async fn init(&mut self) -> Result<&str> {
        if self.upload_id.is_some() {
            return Ok(self.upload_id.as_ref().unwrap());
        }

        debug!("Initializing multipart upload for {}/{}", self.bucket, self.key);

        let upload_id = self
            .client
            .initiate_multipart_upload(&self.bucket, &self.key)
            .await?;

        self.upload_id = Some(upload_id);
        Ok(self.upload_id.as_ref().unwrap())
    }

    pub async fn upload_part(
        &mut self,
        part_number: i32,
        data: Bytes,
    ) -> Result<CompletedPart> {
        if self.completed {
            return Err(ClientError::MultipartAlreadyCompleted);
        }

        let upload_id = match &self.upload_id {
            Some(id) => id,
            None => {
                self.init().await?;
                self.upload_id.as_ref().unwrap()
            }
        };

        let completed_part = self
            .client
            .upload_part(&self.bucket, &self.key, upload_id, part_number, data)
            .await?;

        // Insert the part in order
        if let Some(pos) = self
            .parts
            .iter()
            .position(|p| p.part_number > part_number)
        {
            self.parts.insert(pos, completed_part.clone());
        } else {
            self.parts.push(completed_part.clone());
        }

        Ok(completed_part)
    }

    pub async fn upload_parts<I>(&mut self, parts: I) -> Result<Vec<CompletedPart>>
    where
        I: IntoIterator<Item = (i32, Bytes)>,
    {
        let mut completed = Vec::new();

        for (part_number, data) in parts {
            let part = self.upload_part(part_number, data).await?;
            completed.push(part);
        }

        Ok(completed)
    }

    pub async fn complete(mut self) -> Result<CompleteMultipartOutput> {
        if self.completed {
            return Err(ClientError::MultipartAlreadyCompleted);
        }

        let upload_id = self
            .upload_id
            .as_ref()
            .ok_or(ClientError::MultipartNotInitialized)?;

        if self.parts.is_empty() {
            return Err(ClientError::InvalidParameter {
                parameter: "parts".to_string(),
                value: "empty".to_string(),
            });
        }

        debug!(
            "Completing multipart upload for {}/{} with {} parts",
            self.bucket,
            self.key,
            self.parts.len()
        );

        let result = self
            .client
            .complete_multipart_upload(&self.bucket, &self.key, upload_id, self.parts.clone())
            .await?;

        self.completed = true;

        Ok(result)
    }

    pub async fn abort(mut self) -> Result<()> {
        if self.completed {
            return Err(ClientError::MultipartAlreadyCompleted);
        }

        let upload_id = self
            .upload_id
            .as_ref()
            .ok_or(ClientError::MultipartNotInitialized)?;

        debug!("Aborting multipart upload for {}/{}", self.bucket, self.key);

        self.client
            .abort_multipart_upload(&self.bucket, &self.key, upload_id)
            .await?;

        self.completed = true;

        Ok(())
    }

    pub fn upload_id(&self) -> Option<&str> {
        self.upload_id.as_deref()
    }

    pub fn parts(&self) -> &[CompletedPart] {
        &self.parts
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }
}

#[derive(Debug, Clone)]
pub struct CompletedPart {
    pub part_number: i32,
    pub etag: String,
}

#[derive(Debug)]
pub struct CompleteMultipartOutput {
    pub location: String,
    pub bucket: String,
    pub key: String,
    pub etag: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completed_part() {
        let part = CompletedPart {
            part_number: 1,
            etag: "abc123".to_string(),
        };

        assert_eq!(part.part_number, 1);
        assert_eq!(part.etag, "abc123");
    }
}
