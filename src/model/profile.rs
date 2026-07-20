use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum InstallProfile {
    /// Set up model training with frameworks such as PyTorch, TensorFlow, or JAX.
    ModelTraining,
    /// Set up native CUDA development.
    CudaDevelopment,
}

impl InstallProfile {
    /// Backwards-compatible label used by the interactive profile selector.
    pub fn label(self) -> &'static str {
        self.selection_label()
    }

    pub fn plan_label(self) -> &'static str {
        match self {
            Self::ModelTraining => "Model training (PyTorch, TensorFlow, JAX)",
            Self::CudaDevelopment => "CUDA development",
        }
    }

    pub fn selection_label(self) -> &'static str {
        match self {
            Self::ModelTraining => "Model training     PyTorch, TensorFlow, or JAX",
            Self::CudaDevelopment => "CUDA development   Native CUDA apps and custom kernels",
        }
    }
}
