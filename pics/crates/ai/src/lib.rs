//! IA locale. Rien ici n'ouvre de connexion réseau : les modèles sont **importés** par l'utilisateur depuis
//! un fichier, vérifiés par empreinte SHA-256 avant d'être rangés, et listés avec leur licence.
//!
//! Deux familles de fonctions :
//! - `catalog` : gestion des modèles (catalogue JSON, import vérifié, suppression) ;
//! - `classic` : méthodes **sans modèle** (sélection du sujet par le fond, retouche par diffusion). Elles
//!   ne prétendent pas égaler un réseau de neurones : elles marchent bien sur des fonds simples et de petites
//!   zones à retoucher, et sont déterministes et instantanées.

pub mod catalog;
pub mod classic;
#[cfg(feature = "onnx")]
pub mod onnx;

pub use catalog::{AiError, ModelEntry, ModelStore, Task};
