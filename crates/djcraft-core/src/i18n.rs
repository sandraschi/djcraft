//! DJCraft Internationalization (i18n) Engine.
//!
//! Follows the fleet standard: typed locale enums, zero-panic string lookups,
//! multi-language translations (EN, DE, FR, ES, JA, KO, ZH),
//! and DJ performance / audio DSP terminology localization.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppLocale {
    #[default]
    En, // English (Global Default)
    De, // German (Berlin Techno native)
    Fr, // French (French Touch native)
    Es, // Spanish (Ibiza / Latin electronic native)
    Ja, // Japanese (Tokyo club / J-Core native)
    Ko, // Korean (K-EDM native)
    Zh, // Chinese
}

impl AppLocale {
    pub fn all() -> &'static [AppLocale] {
        &[
            AppLocale::En,
            AppLocale::De,
            AppLocale::Fr,
            AppLocale::Es,
            AppLocale::Ja,
            AppLocale::Ko,
            AppLocale::Zh,
        ]
    }

    pub fn code(&self) -> &'static str {
        match self {
            AppLocale::En => "en",
            AppLocale::De => "de",
            AppLocale::Fr => "fr",
            AppLocale::Es => "es",
            AppLocale::Ja => "ja",
            AppLocale::Ko => "ko",
            AppLocale::Zh => "zh",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AppLocale::En => "English (US/UK)",
            AppLocale::De => "Deutsch (DE/AT/CH)",
            AppLocale::Fr => "Français (FR)",
            AppLocale::Es => "Español (ES/LATAM)",
            AppLocale::Ja => "日本語 (JA)",
            AppLocale::Ko => "한국어 (KO)",
            AppLocale::Zh => "中文 (ZH)",
        }
    }

    pub fn flag(&self) -> &'static str {
        match self {
            AppLocale::En => "🇬🇧",
            AppLocale::De => "🇩🇪",
            AppLocale::Fr => "🇫🇷",
            AppLocale::Es => "🇪🇸",
            AppLocale::Ja => "🇯🇵",
            AppLocale::Ko => "🇰🇷",
            AppLocale::Zh => "🇨🇳",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum I18nKey {
    // Decks & Controls
    DeckA,
    DeckB,
    Crossfader,
    BpmSync,
    LoopRoll,
    HotCue,
    StemSeparation,
    VinylScratch,
    Equalizer,
    StatusReady,
}

pub fn t(key: I18nKey, locale: AppLocale) -> &'static str {
    match (key, locale) {
        (I18nKey::DeckA, AppLocale::En) => "Deck A (Left)",
        (I18nKey::DeckA, AppLocale::De) => "Deck A (Links)",
        (I18nKey::DeckA, AppLocale::Fr) => "Platine A (Gauche)",
        (I18nKey::DeckA, AppLocale::Es) => "Plato A (Izquierda)",
        (I18nKey::DeckA, AppLocale::Ja) => "デッキ A (左側)",
        (I18nKey::DeckA, AppLocale::Ko) => "데크 A (왼쪽)",
        (I18nKey::DeckA, AppLocale::Zh) => "唱盘 A (左轨)",

        (I18nKey::DeckB, AppLocale::En) => "Deck B (Right)",
        (I18nKey::DeckB, AppLocale::De) => "Deck B (Rechts)",
        (I18nKey::DeckB, AppLocale::Fr) => "Platine B (Droite)",
        (I18nKey::DeckB, AppLocale::Es) => "Plato B (Derecha)",
        (I18nKey::DeckB, AppLocale::Ja) => "デッキ B (右側)",
        (I18nKey::DeckB, AppLocale::Ko) => "데크 B (오른쪽)",
        (I18nKey::DeckB, AppLocale::Zh) => "唱盘 B (右轨)",

        (I18nKey::Crossfader, AppLocale::En) => "Crossfader Curve",
        (I18nKey::Crossfader, AppLocale::De) => "Crossfader-Kurve",
        (I18nKey::Crossfader, AppLocale::Fr) => "Courbe de crossfader",
        (I18nKey::Crossfader, AppLocale::Es) => "Curva de crossfader",
        (I18nKey::Crossfader, AppLocale::Ja) => "クロスフェーダー特性",
        (I18nKey::Crossfader, AppLocale::Ko) => "크로스페이더 곡선",
        (I18nKey::Crossfader, AppLocale::Zh) => "交叉推子响应曲线",

        (I18nKey::BpmSync, AppLocale::En) => "BPM Grid Sync",
        (I18nKey::BpmSync, AppLocale::De) => "BPM-Raster-Sync",
        (I18nKey::BpmSync, AppLocale::Fr) => "Sync grille BPM",
        (I18nKey::BpmSync, AppLocale::Es) => "Sincronización de BPM",
        (I18nKey::BpmSync, AppLocale::Ja) => "BPM グリッド同期 (SYNC)",
        (I18nKey::BpmSync, AppLocale::Ko) => "BPM 그리드 동기화",
        (I18nKey::BpmSync, AppLocale::Zh) => "BPM 节拍网格同步",

        (I18nKey::LoopRoll, AppLocale::En) => "Loop Roll",
        (I18nKey::LoopRoll, AppLocale::De) => "Loop-Roll",
        (I18nKey::LoopRoll, AppLocale::Fr) => "Boucle Roll",
        (I18nKey::LoopRoll, AppLocale::Es) => "Loop Roll",
        (I18nKey::LoopRoll, AppLocale::Ja) => "ループ・ロール",
        (I18nKey::LoopRoll, AppLocale::Ko) => "루프 롤",
        (I18nKey::LoopRoll, AppLocale::Zh) => "循环跳音 (Loop Roll)",

        (I18nKey::HotCue, AppLocale::En) => "Hot Cues",
        (I18nKey::HotCue, AppLocale::De) => "Hot-Cues",
        (I18nKey::HotCue, AppLocale::Fr) => "Points Cue",
        (I18nKey::HotCue, AppLocale::Es) => "Hot Cues",
        (I18nKey::HotCue, AppLocale::Ja) => "ホットキュー",
        (I18nKey::HotCue, AppLocale::Ko) => "핫 큐",
        (I18nKey::HotCue, AppLocale::Zh) => "快捷定位点 (Hot Cue)",

        (I18nKey::StemSeparation, AppLocale::En) => "AI Stem Separation (Vocals, Drums, Bass, Melodic)",
        (I18nKey::StemSeparation, AppLocale::De) => "KI-Stemtrennung (Gesang, Drums, Bass, Melodie)",
        (I18nKey::StemSeparation, AppLocale::Fr) => "Séparation des pistes IA (Voix, Batterie, Basse)",
        (I18nKey::StemSeparation, AppLocale::Es) => "Separación de pistas por IA (Voces, Batería, Bajo)",
        (I18nKey::StemSeparation, AppLocale::Ja) => "AI音源分離 (ボーカル・ドラム・ベース・メロディ)",
        (I18nKey::StemSeparation, AppLocale::Ko) => "AI 파트 분리 (보컬, 드럼, 베이스, 멜로디)",
        (I18nKey::StemSeparation, AppLocale::Zh) => "AI 分轨拆分 (人声、鼓点、贝斯、旋律)",

        (I18nKey::VinylScratch, AppLocale::En) => "Vinyl Jog Scratch",
        (I18nKey::VinylScratch, AppLocale::De) => "Vinyl-Jog-Scratch",
        (I18nKey::VinylScratch, AppLocale::Fr) => "Scratch vinyle jog",
        (I18nKey::VinylScratch, AppLocale::Es) => "Scratch de vinilo",
        (I18nKey::VinylScratch, AppLocale::Ja) => "ヴァイナル・ジョグスクラッチ",
        (I18nKey::VinylScratch, AppLocale::Ko) => "바이닐 조그 스크래치",
        (I18nKey::VinylScratch, AppLocale::Zh) => "黑胶转盘搓碟 (Scratch)",

        (I18nKey::Equalizer, AppLocale::En) => "3-Band Isolator EQ (Hi, Mid, Low)",
        (I18nKey::Equalizer, AppLocale::De) => "3-Band Isolator-EQ (Höhen, Mitten, Bässe)",
        (I18nKey::Equalizer, AppLocale::Fr) => "Égaliseur isolateur 3 bandes",
        (I18nKey::Equalizer, AppLocale::Es) => "Ecualizador aislador de 3 bandas",
        (I18nKey::Equalizer, AppLocale::Ja) => "3バンド アイソレーターEQ",
        (I18nKey::Equalizer, AppLocale::Ko) => "3밴드 아이솔레이터 EQ",
        (I18nKey::Equalizer, AppLocale::Zh) => "三段式硬件隔离均衡器 (EQ)",

        (I18nKey::StatusReady, AppLocale::En) => "DJCraft Real-Time Audio DSP Ready",
        (I18nKey::StatusReady, AppLocale::De) => "DJCraft Echtzeit-Audio-DSP bereit",
        (I18nKey::StatusReady, AppLocale::Fr) => "DSP audio temps réel DJCraft prêt",
        (I18nKey::StatusReady, AppLocale::Es) => "DSP de audio en tiempo real DJCraft listo",
        (I18nKey::StatusReady, AppLocale::Ja) => "DJCraft リアルタイム音響DSP 準備完了",
        (I18nKey::StatusReady, AppLocale::Ko) => "DJCraft 실시간 오디오 DSP 준비완료",
        (I18nKey::StatusReady, AppLocale::Zh) => "DJCraft 实时音频数字信号处理器 (DSP) 就绪",
    }
}
