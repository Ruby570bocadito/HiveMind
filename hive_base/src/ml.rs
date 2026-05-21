use ndarray::Array2;
use ort::session::Session;
use ort::value::Value;
use std::sync::Once;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MlError {
    #[error("ONNX Runtime Error: {0}")]
    OrtError(#[from] ort::Error),
    #[error("Model load error: {0}")]
    LoadError(String),
}

static INIT_ORT: Once = Once::new();

pub fn init_ort() {
    INIT_ORT.call_once(|| {
        let _ = ort::init().with_name("malware_swarm").commit();
    });
}

pub struct OnnxModel {
    session: Session,
}

impl OnnxModel {
    pub fn new(model_bytes: &[u8]) -> Result<Self, MlError> {
        init_ort();
        let session = Session::builder()?
            .commit_from_memory(model_bytes)?;
            
        Ok(Self { session })
    }

    pub fn predict_f32(&mut self, input: Array2<f32>) -> Result<Vec<f32>, MlError> {
        let shape = vec![input.shape()[0], input.shape()[1]];
        let input_tensor = Value::from_array((shape, input.into_raw_vec()))?;
        let outputs = self.session.run(ort::inputs![input_tensor])?;
        
        // Take first output
        let output = &outputs[0];
        let output_view = output.try_extract_tensor::<f32>()?;
        
        Ok(output_view.1.to_vec())
    }

    pub fn predict_i64(&mut self, input: Array2<f32>) -> Result<Vec<i64>, MlError> {
        let shape = vec![input.shape()[0], input.shape()[1]];
        let input_tensor = Value::from_array((shape, input.into_raw_vec()))?;
        let outputs = self.session.run(ort::inputs![input_tensor])?;
        
        let output = &outputs[0];
        let output_view = output.try_extract_tensor::<i64>()?;
        
        Ok(output_view.1.to_vec())
    }
}
