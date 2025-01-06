import numpy as np
import asyncio
from holochain_client import HolochainClient
from dataclasses import dataclass
from typing import List, Dict, Optional
import logging
import ssl
import hashlib

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

@dataclass
class ModelMetrics:
    loss: float
    accuracy: float
    samples_count: int

@dataclass
class ModelUpdate:
    weights: np.ndarray
    bias: float
    version: int
    metrics: ModelMetrics

class FederatedLearningClient:
    def __init__(
        self,
        conductor_url: str,
        learning_rate: float = 0.01,
        batch_size: int = 32,
        epochs: int = 5,
    ):
        self.client = HolochainClient(conductor_url)
        self.local_model: Optional[Dict] = None
        self.learning_rate = learning_rate
        self.batch_size = batch_size
        self.epochs = epochs
        self.version = 0
        self.ssl_context = ssl.create_default_context()
        logger.info("FederatedLearningClient initialized")

    async def connect(self):
        """Initialize connection to Holochain conductor"""
        try:
            await self.client.connect(ssl_context=self.ssl_context)
            logger.info("Connected to Holochain conductor")
        except Exception as e:
            logger.error(f"Failed to connect to Holochain conductor: {e}")
            raise

    def initialize_model(self, input_size: int):
        """Initialize local model parameters"""
        self.local_model = {
            'weights': np.random.randn(input_size) * 0.01,
            'bias': 0.0
        }
        logger.info(f"Initialized model with input size: {input_size}")

    def _compute_metrics(self, X: np.ndarray, y: np.ndarray) -> ModelMetrics:
        """Compute model metrics"""
        predictions = self._predict(X)
        loss = np.mean((predictions - y) ** 2)
        accuracy = np.mean((predictions > 0.5) == y)
        return ModelMetrics(
            loss=float(loss),
            accuracy=float(accuracy),
            samples_count=len(X)
        )

    def _predict(self, X: np.ndarray) -> np.ndarray:
        """Make predictions using local model"""
        return 1 / (1 + np.exp(-np.dot(X, self.local_model['weights']) - self.local_model['bias']))

    async def train_local_model(self, X: np.ndarray, y: np.ndarray):
        """Train local model on client data"""
        if self.local_model is None:
            self.initialize_model(X.shape[1])
        logger.info("Starting local training...")
        for epoch in range(self.epochs):
            indices = np.random.permutation(len(X))
            epoch_loss = 0
            for i in range(0, len(X), self.batch_size):
                batch_indices = indices[i:i + self.batch_size]
                X_batch = X[batch_indices]
                y_batch = y[batch_indices]
                # Forward pass
                predictions = self._predict(X_batch)
                loss = np.mean((predictions - y_batch) ** 2)
                epoch_loss += loss
                # Backward pass
                error = predictions - y_batch
                weight_gradients = np.dot(X_batch.T, error) / len(X_batch)
                bias_gradient = np.mean(error)
                # Update parameters
                self.local_model['weights'] -= self.learning_rate * weight_gradients
                self.local_model['bias'] -= self.learning_rate * bias_gradient
            avg_loss = epoch_loss / (len(X) / self.batch_size)
            logger.info(f"Epoch {epoch + 1}/{self.epochs}, Loss: {avg_loss:.4f}")
        metrics = self._compute_metrics(X, y)
        logger.info(f"Training completed. Final metrics: {metrics}")

    async def submit_update(self, X: np.ndarray, y: np.ndarray):
        """Submit local model update to Holochain network"""
        try:
            metrics = self._compute_metrics(X, y)
            update = ModelUpdate(
                weights=self.local_model['weights'],
                bias=self.local_model['bias'],
                version=self.version + 1,
                metrics=metrics
            )
            update_hash = self._hash_update(update)
            await self.client.call("submit_model_update", update)
            logger.info(f"Submitted model update version {update.version} with hash {update_hash}")
        except Exception as e:
            logger.error(f"Failed to submit model update: {e}")
            raise

    def _hash_update(self, update: ModelUpdate) -> str:
        """Compute the hash of the model update"""
        hasher = hashlib.sha256()
        hasher.update(update.weights.tobytes())
        hasher.update(update.bias.tobytes())
        hasher.update(update.version.to_bytes(4, 'little'))
        hasher.update(update.metrics.loss.tobytes())
        hasher.update(update.metrics.accuracy.tobytes())
        hasher.update(update.metrics.samples_count.to_bytes(4, 'little'))
        return hasher.hexdigest()