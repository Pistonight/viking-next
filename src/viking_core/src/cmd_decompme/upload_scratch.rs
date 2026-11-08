use colored::Colorize;
use cu::pre::*;
use reqwest::blocking::Client;

use crate::config::{ConfigDecompMe, ConfigDecompMeTarget};
use crate::ui;

use crate::cmd_decompme::Args;

/// Upload scratch to decompme and return the uploaded information
pub fn run_upload(
    config: &ConfigDecompMe,
    args: &Args,
    mut source: ScratchSource,
) -> cu::Result<FinalScratchUrl> {
    let override_compile_flags = config.override_compile_flags;
    let client = Client::new();

    let platform = "switch".to_string();

    // decomp.me behavior regarding compiler, compiler_flags and preset_id:
    // - if preset_id is specified, then the compiler in the request is ignored
    // - if preset_id is specified, compiler_flags in the request still overrides
    //   the flags in the preset
    //
    // we print warnings considering the input compiler_flags in `source`
    // (discovered from compilation database) and the `override_compile_flags`
    // config option (allow overriding from compilation database)
    let (compiler, preset) = match &config.target {
        ConfigDecompMeTarget::Preset { preset_id } => {
            match (override_compile_flags, &source.compiler_flags) {
                // most common cases:
                // - default config + flags found in compilation database
                // - config explicitly disallows override + flags not found (use configured preset_id or
                // flags)
                (true, Some(_)) | (false, None) => {}
                // flags were not found but overriding is allowed
                (true, None) => {
                    ui::print_warning(
                        "did not find compile flags; falling back to the configured preset_id",
                    );
                }
                (false, Some(_)) => {
                    ui::print_warning(
                        "decompe_me.override_compile_flags is false - not using the discovered compile flags; using the configured preset_id instead",
                    );
                    source.compiler_flags = None;
                }
            }
            (None, Some(preset_id.to_string()))
        }
        ConfigDecompMeTarget::CompilerAndFlags {
            compiler_name,
            default_compile_flags,
        } => {
            match (override_compile_flags, &source.compiler_flags) {
                // most common case, default config + flags found in compilation database
                (true, Some(_)) => {}
                // config disallows override + did not discover compile flags
                (false, None) => {
                    source.compiler_flags = Some(default_compile_flags.to_string());
                }
                (true, None) => {
                    ui::print_warning(
                        "did not find compile flags; falling back to the configured default_compile_flags",
                    );
                    source.compiler_flags = Some(default_compile_flags.to_string());
                }
                (false, Some(discovered_flags)) => {
                    if discovered_flags.trim() != default_compile_flags.trim() {
                        ui::print_warning(
                            "decompe_me.override_compile_flags is false - not using the discovered compile flags; using the configured default_compile_flags instead",
                        );
                        source.compiler_flags = Some(default_compile_flags.to_string());
                    }
                }
            }
            (Some(compiler_name.to_string()), None)
        }
    };

    let request = ScratchRequest {
        source,
        platform,
        compiler,
        preset,
    };

    let api_base = &args.decomp_me_api;

    let response = cu::check!(
        client
            .post(format!("{api_base}/api/scratch"))
            .json(&request)
            .send(),
        "failed to send request"
    )?;
    let response = cu::check!(response.text(), "failed to read response text")?;
    let response = match json::parse::<ScratchResponse>(&response) {
        Ok(x) => x,
        Err(e) => {
            ui::print_note(&format!(
                "server response:\n{}\n",
                &response.normal().yellow()
            ));
            cu::rethrow!(e, "failed to upload function");
        }
    };
    let ScratchResponse { slug, claim_token } = response;

    let base_url = format!("{api_base}/scratch/{slug}/");
    let claim_url = format!("{api_base}/scratch/{slug}/claim?token={claim_token}");

    Ok(FinalScratchUrl {
        base_url,
        claim_url,
    })
}

/// Source information to create the scratch
#[derive(Debug, Serialize)]
pub struct ScratchSource {
    /// Name for display (The demangled name)
    pub name: String,
    /// Target disassembly
    pub target_asm: String,
    /// Content of the "Source code" tab (The function being worked on)
    pub source_code: String,
    /// Content of the "Context" tab (Preprocessed context needed to compile the source)
    pub context: String,
    /// Label for diffing (mangled name)
    pub diff_label: String,
    /// Flags for asm-differ
    pub diff_flags: Vec<String>,
    /// Compiler flags to use instead of default (if decomp_me.override_compile_flags is true in the config)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_flags: Option<String>,
}

/// Upload request JSON
#[derive(Debug, Serialize)]
pub struct ScratchRequest {
    #[serde(flatten)]
    source: ScratchSource,

    /// "switch"
    platform: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    compiler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preset: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct ScratchResponse {
    slug: String,
    claim_token: String,
}

#[derive(Debug, Default, Clone)]
pub struct FinalScratchUrl {
    pub base_url: String,
    pub claim_url: String,
}
