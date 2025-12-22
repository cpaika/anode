use crate::client::AnodeClient;
use crate::error::{ClientError, Result};
use bytes::Bytes;
use tracing::debug;

/// A multipart upload for uploading large objects in parts.
///
/// Multipart uploads allow you to upload large objects (> 5MB) in smaller chunks,
/// which can be uploaded in parallel for better performance and reliability. Each
/// part must be at least 5MB in size (except the last part).
///
/// # Workflow
///
/// 1. Create a [`MultipartUpload`] using [`AnodeClient::multipart_upload`]
/// 2. Initialize the upload with [`init`](MultipartUpload::init)
/// 3. Upload parts with [`upload_part`](MultipartUpload::upload_part)
/// 4. Complete the upload with [`complete`](MultipartUpload::complete) or abort with [`abort`](MultipartUpload::abort)
///
/// # Example
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use bytes::Bytes;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ClientConfig::new("http://localhost:8080");
/// let client = AnodeClient::new(config)?;
///
/// let mut upload = client.multipart_upload("my-bucket", "large-file.bin");
/// upload.init().await?;
///
/// // Upload parts (each part must be at least 5MB except the last)
/// upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
/// upload.upload_part(2, Bytes::from(vec![1u8; 5_242_880])).await?;
/// upload.upload_part(3, Bytes::from(vec![2u8; 1_024_000])).await?;
///
/// let result = upload.complete().await?;
/// println!("Upload complete: {}", result.etag);
/// # Ok(())
/// # }
/// ```
pub struct MultipartUpload<'a> {
    client: &'a AnodeClient,
    bucket: String,
    key: String,
    upload_id: Option<String>,
    parts: Vec<CompletedPart>,
    completed: bool,
}

impl<'a> MultipartUpload<'a> {
    /// Creates a new multipart upload (internal use only).
    ///
    /// Use [`AnodeClient::multipart_upload`] instead.
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

    /// Initializes the multipart upload.
    ///
    /// This must be called before uploading any parts. If the upload is already
    /// initialized, this returns the existing upload ID.
    ///
    /// # Returns
    ///
    /// The upload ID string that can be used to reference this upload.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "file.bin");
    /// let upload_id = upload.init().await?;
    /// println!("Upload ID: {}", upload_id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn init(&mut self) -> Result<&str> {
        if self.upload_id.is_some() {
            return Ok(self.upload_id.as_ref().unwrap());
        }

        debug!(
            "Initializing multipart upload for {}/{}",
            self.bucket, self.key
        );

        let upload_id = self
            .client
            .initiate_multipart_upload(&self.bucket, &self.key)
            .await?;

        self.upload_id = Some(upload_id);
        Ok(self.upload_id.as_ref().unwrap())
    }

    /// Uploads a single part of the multipart upload.
    ///
    /// Parts are numbered from 1 to 10000. Each part must be at least 5MB in size
    /// except the last part. Parts can be uploaded in any order and in parallel.
    ///
    /// If the upload hasn't been initialized yet, this will automatically call [`init`](Self::init).
    ///
    /// # Arguments
    ///
    /// * `part_number` - Part number (1-10000)
    /// * `data` - Part data (at least 5MB except for the last part)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The part number is not between 1 and 10000
    /// - The upload has already been completed
    /// - Network error occurs
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "file.bin");
    /// upload.init().await?;
    ///
    /// let part1 = upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
    /// let part2 = upload.upload_part(2, Bytes::from(vec![1u8; 5_242_880])).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn upload_part(&mut self, part_number: i32, data: Bytes) -> Result<CompletedPart> {
        if self.completed {
            return Err(ClientError::MultipartAlreadyCompleted);
        }

        let upload_id = match &self.upload_id {
            Some(id) => id,
            None => {
                self.init().await?;
                self.upload_id.as_ref().unwrap()
            },
        };

        let completed_part = self
            .client
            .upload_part(&self.bucket, &self.key, upload_id, part_number, data)
            .await?;

        // Insert the part in order
        if let Some(pos) = self.parts.iter().position(|p| p.part_number > part_number) {
            self.parts.insert(pos, completed_part.clone());
        } else {
            self.parts.push(completed_part.clone());
        }

        Ok(completed_part)
    }

    /// Uploads multiple parts sequentially.
    ///
    /// This is a convenience method for uploading multiple parts. For better performance,
    /// consider uploading parts in parallel using multiple calls to [`upload_part`](Self::upload_part).
    ///
    /// # Arguments
    ///
    /// * `parts` - Iterator of (part_number, data) tuples
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "file.bin");
    /// upload.init().await?;
    ///
    /// let parts = vec![
    ///     (1, Bytes::from(vec![0u8; 5_242_880])),
    ///     (2, Bytes::from(vec![1u8; 5_242_880])),
    /// ];
    ///
    /// upload.upload_parts(parts).await?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Completes the multipart upload.
    ///
    /// This assembles all uploaded parts into a single object. After completion,
    /// no more parts can be uploaded.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The upload hasn't been initialized
    /// - No parts have been uploaded
    /// - The upload has already been completed
    /// - Network error occurs
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "file.bin");
    /// upload.init().await?;
    /// upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
    ///
    /// let result = upload.complete().await?;
    /// println!("Upload complete! ETag: {}", result.etag);
    /// # Ok(())
    /// # }
    /// ```
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

    /// Aborts the multipart upload.
    ///
    /// This cancels the upload and frees all storage used by the uploaded parts.
    /// After aborting, the upload cannot be resumed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The upload hasn't been initialized
    /// - The upload has already been completed
    /// - Network error occurs
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "file.bin");
    /// upload.init().await?;
    /// upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
    ///
    /// // Something went wrong, abort the upload
    /// upload.abort().await?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Returns the upload ID if the upload has been initialized.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "file.bin");
    /// assert!(upload.upload_id().is_none());
    ///
    /// upload.init().await?;
    /// assert!(upload.upload_id().is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub fn upload_id(&self) -> Option<&str> {
        self.upload_id.as_deref()
    }

    /// Returns the list of completed parts.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "file.bin");
    /// upload.init().await?;
    /// upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
    ///
    /// assert_eq!(upload.parts().len(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn parts(&self) -> &[CompletedPart] {
        &self.parts
    }

    /// Returns `true` if the upload has been completed or aborted.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "file.bin");
    /// assert!(!upload.is_completed());
    ///
    /// upload.init().await?;
    /// upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
    /// upload.complete().await?;
    /// // Can't check is_completed() here because upload was consumed
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_completed(&self) -> bool {
        self.completed
    }
}

/// Information about a completed multipart upload part.
///
/// Each uploaded part is identified by its part number and ETag.
#[derive(Debug, Clone)]
pub struct CompletedPart {
    /// Part number (1-10000)
    pub part_number: i32,
    /// ETag returned by the server for this part
    pub etag: String,
}

/// Result of a completed multipart upload.
///
/// Contains the location, bucket, key, and ETag of the assembled object.
#[derive(Debug)]
pub struct CompleteMultipartOutput {
    /// URL location of the uploaded object
    pub location: String,
    /// Bucket name
    pub bucket: String,
    /// Object key
    pub key: String,
    /// ETag of the assembled object
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
