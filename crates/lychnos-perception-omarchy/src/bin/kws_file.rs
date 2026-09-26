use std::{env, path::PathBuf};

use sherpa_onnx::{KeywordSpotter, KeywordSpotterConfig, Wave};

fn main() -> Result<(), String> {
    let wav = env::args().nth(1).ok_or("usage: kws_file WAV [KEYWORDS]")?;
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let model = home
        .join(".local/share/lychnos/models/voice-v2/sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20");
    let keywords = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| model.join("lychnos-keywords.txt"));

    let mut config = KeywordSpotterConfig::default();
    config.model_config.transducer.encoder = Some(
        model
            .join("encoder-epoch-13-avg-2-chunk-8-left-64.int8.onnx")
            .display()
            .to_string(),
    );
    config.model_config.transducer.decoder = Some(
        model
            .join("decoder-epoch-13-avg-2-chunk-8-left-64.onnx")
            .display()
            .to_string(),
    );
    config.model_config.transducer.joiner = Some(
        model
            .join("joiner-epoch-13-avg-2-chunk-8-left-64.int8.onnx")
            .display()
            .to_string(),
    );
    config.model_config.tokens = Some(model.join("tokens.txt").display().to_string());
    config.model_config.provider = Some("cpu".into());
    config.model_config.num_threads = 1;
    config.max_active_paths = 4;
    config.num_trailing_blanks = 1;
    config.keywords_score = 3.0;
    config.keywords_threshold = 0.10;
    config.keywords_file = Some(keywords.display().to_string());

    let kws = KeywordSpotter::create(&config).ok_or("failed to create KWS")?;
    let wave = Wave::read(&wav).ok_or_else(|| format!("failed to read {wav}"))?;
    let stream = kws.create_stream();
    stream.accept_waveform(wave.sample_rate(), wave.samples());
    stream.accept_waveform(
        wave.sample_rate(),
        &vec![0.0; (wave.sample_rate() / 2) as usize],
    );
    stream.input_finished();

    let mut found = false;
    while kws.is_ready(&stream) {
        kws.decode(&stream);
        if let Some(result) = kws.get_result(&stream)
            && !result.keyword.is_empty()
        {
            println!("{}", result.json);
            found = true;
            kws.reset(&stream);
        }
    }

    if !found {
        println!("NO_KEYWORD");
    }
    Ok(())
}
