#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::*;

    // ============================================================
    // FeatureType tests
    // ============================================================

    #[test]
    fn test_featuretype_toggle_as_bool() {
        let toggle = FeatureType::Toggle(true);
        assert_eq!(toggle.as_bool(), Some(true));

        let toggle_off = FeatureType::Toggle(false);
        assert_eq!(toggle_off.as_bool(), Some(false));
    }

    #[test]
    fn test_featuretype_slider_as_f32() {
        let slider = FeatureType::Slider(0.5);
        assert_eq!(slider.as_f32(), Some(0.5));

        let slider_zero = FeatureType::Slider(0.0);
        assert_eq!(slider_zero.as_f32(), Some(0.0));
    }

    #[test]
    fn test_featuretype_toggle_not_slider() {
        let toggle = FeatureType::Toggle(true);
        assert_eq!(toggle.as_f32(), None);
    }

    #[test]
    fn test_featuretype_slider_not_toggle() {
        let slider = FeatureType::Slider(0.5);
        assert_eq!(slider.as_bool(), None);
    }

    // ============================================================
    // Format enum tests
    // ============================================================

    #[test]
    fn test_format_global() {
        let fmt = Format::Global(0x01);
        assert_eq!(format!("{:?}", fmt), "Global(1)");
    }

    #[test]
    fn test_format_sbx() {
        let fmt = Format::SBX(0x00);
        assert_eq!(format!("{:?}", fmt), "SBX(0)");
    }

    #[test]
    fn test_format_rgb() {
        let fmt = Format::RGB(0x01);
        assert_eq!(format!("{:?}", fmt), "RGB(1)");
    }

    #[test]
    fn test_format_routing() {
        let fmt = Format::Routing(0x02);
        assert_eq!(format!("{:?}", fmt), "Routing(2)");
    }

    // ============================================================
    // value_to_bytes tests
    // ============================================================

    #[test]
    fn test_value_to_bytes_zero() {
        let bytes = value_to_bytes(0);
        assert_eq!(bytes, 0.0f32.to_le_bytes());
    }

    #[test]
    fn test_value_to_bytes_full() {
        let bytes = value_to_bytes(100);
        assert_eq!(bytes, 1.0f32.to_le_bytes());
    }

    #[test]
    fn test_value_to_bytes_half() {
        let bytes = value_to_bytes(50);
        assert_eq!(bytes, 0.5f32.to_le_bytes());
    }

    // ============================================================
    // create_payload tests for G6X Global (Scout Mode)
    // ============================================================

    #[test]
    fn test_create_payload_global_scout_mode_on() {
        let payload = create_payload(Format::Global(0x02), 1.0);

        // Byte 0: HID Report ID
        assert_eq!(payload.data[0], 0x00);
        // Byte 1: Magic
        assert_eq!(payload.data[1], 0x5a);
        // Byte 2: Class 0x26
        assert_eq!(payload.data[2], 0x26);
        // Byte 3: 0x05
        assert_eq!(payload.data[3], 0x05);
        // Byte 4: 0x07
        assert_eq!(payload.data[4], 0x07);
        // Byte 5: feature ID (0x02 = Scout Mode)
        assert_eq!(payload.data[5], 0x02);
        // Byte 6: 0x00
        assert_eq!(payload.data[6], 0x00);
        // Byte 7: value (1.0 → 0x01)
        assert_eq!(payload.data[7], 0x01);
    }

    #[test]
    fn test_create_payload_global_scout_mode_off() {
        let payload = create_payload(Format::Global(0x02), 0.0);

        // Byte 7: value (0.0 → 0x00)
        assert_eq!(payload.data[7], 0x00);
    }

    #[test]
    fn test_create_payload_global_commit() {
        let payload = create_payload(Format::Global(0x02), 1.0);

        // COMMIT command for Global format
        // Byte 2: 0x26
        assert_eq!(payload.commit[2], 0x26);
        // Byte 3: 0x03
        assert_eq!(payload.commit[3], 0x03);
        // Byte 4: 0x08
        assert_eq!(payload.commit[4], 0x08);
        // Byte 5: 0xff
        assert_eq!(payload.commit[5], 0xff);
        // Byte 6: 0xff (magic commit value)
        assert_eq!(payload.commit[6], 0xff);
    }

    // ============================================================
    // create_payload tests for G6X SBX audio features
    // ============================================================

    #[test]
    fn test_create_payload_sbx_eq_toggle() {
        // EQ toggle (0x09) uses normalized value, not raw dB
        let payload = create_payload(Format::SBX(0x09), 100.0);

        // Byte 2: 0x12 (SET class)
        assert_eq!(payload.data[2], 0x12);
        // Byte 3: 0x07 (length)
        assert_eq!(payload.data[3], 0x07);
        // Byte 4: 0x01 (feature high)
        assert_eq!(payload.data[4], 0x01);
        // Byte 5: 0x96 (feature low)
        assert_eq!(payload.data[5], 0x96);
        // Byte 6: sub-ID 0x09 (EQ toggle)
        assert_eq!(payload.data[6], 0x09);
        // Bytes 7-10: value as float (100.0 / 100.0 = 1.0)
        assert_eq!(payload.data[7..11], 1.0f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_sbx_eq_band() {
        // EQ band (0x0b-0x14) uses raw float value, not normalized
        let payload = create_payload(Format::SBX(0x0b), 3.5);

        // Bytes 7-10: raw value 3.5 as float
        assert_eq!(payload.data[7..11], 3.5f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_sbx_eq_band_negative() {
        // EQ band can be negative
        let payload = create_payload(Format::SBX(0x0b), -6.0);

        assert_eq!(payload.data[7..11], (-6.0f32).to_le_bytes());
    }

    #[test]
    fn test_create_payload_sbx_surround() {
        let payload = create_payload(Format::SBX(0x00), 100.0);

        // sub-ID 0x00 = Surround
        assert_eq!(payload.data[6], 0x00);
        // Value normalized to 1.0
        assert_eq!(payload.data[7..11], 1.0f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_sbx_commit() {
        let payload = create_payload(Format::SBX(0x00), 100.0);

        // COMMIT for SBX uses class 0x11
        assert_eq!(payload.commit[2], 0x11);
        // Byte 3: 0x03 (length)
        assert_eq!(payload.commit[3], 0x03);
        // Byte 4: 0x01 (feature high)
        assert_eq!(payload.commit[4], 0x01);
        // Byte 5: 0x96 (feature low)
        assert_eq!(payload.commit[5], 0x96);
        // Byte 6: sub-ID
        assert_eq!(payload.commit[6], 0x00);
    }

    // ============================================================
    // create_payload tests for G6X Routing (Output Mode)
    // ============================================================

    #[test]
    fn test_create_payload_routing_headphones() {
        let payload = create_payload(Format::Routing(0x02), 1.0);

        // Byte 2: 0x2c (routing class)
        assert_eq!(payload.data[2], 0x2c);
        // Byte 3: feature ID
        assert_eq!(payload.data[3], 0x02);
        // Byte 4: 0x00
        assert_eq!(payload.data[4], 0x00);
        // Byte 5: 0x04 (headphones)
        assert_eq!(payload.data[5], 0x04);
    }

    #[test]
    fn test_create_payload_routing_speakers() {
        let payload = create_payload(Format::Routing(0x02), 0.0);

        // Byte 5: 0x02 (speakers)
        assert_eq!(payload.data[5], 0x02);
    }

    // ============================================================
    // create_payload tests for G8 Acoustic Engine
    // ============================================================

    #[test]
    fn test_create_payload_g8_ae_eq_toggle() {
        // EQ toggle: Format::AcousticEngine(0x09)
        let payload = create_payload(Format::AcousticEngine(0x09), 1.0);

        // Byte 2: 0x12 (SET class)
        assert_eq!(payload.data[2], 0x12);
        // Byte 3: 0x07 (length)
        assert_eq!(payload.data[3], 0x07);
        // Byte 4: 0x01 (feature high)
        assert_eq!(payload.data[4], 0x01);
        // Byte 5: 0x96 (feature low — Acoustic Engine)
        assert_eq!(payload.data[5], 0x96);
        // Byte 6: sub-ID 0x09 (EQ toggle)
        assert_eq!(payload.data[6], 0x09);
        // Bytes 7-10: value as float (IEEE 754 LE)
        assert_eq!(payload.data[7..11], 1.0f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_g8_ae_eq_band() {
        // EQ band: Format::AcousticEngine(0x0b)
        let payload = create_payload(Format::AcousticEngine(0x0b), 4.5);

        // Bytes 7-10: raw float value
        assert_eq!(payload.data[7..11], 4.5f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_g8_ae_eq_band_negative() {
        // EQ band can be negative
        let payload = create_payload(Format::AcousticEngine(0x0b), -6.0);

        assert_eq!(payload.data[7..11], (-6.0f32).to_le_bytes());
    }

    #[test]
    fn test_create_payload_g8_ae_preamp() {
        // Preamp: Format::AcousticEngine(0x0a)
        let payload = create_payload(Format::AcousticEngine(0x0a), 3.0);

        // Byte 6: sub-ID 0x0a (Preamp)
        assert_eq!(payload.data[6], 0x0a);
        // Bytes 7-10: value
        assert_eq!(payload.data[7..11], 3.0f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_g8_ae_surround() {
        // Surround: Format::AcousticEngine(0x00)
        let payload = create_payload(Format::AcousticEngine(0x00), 1.0);

        assert_eq!(payload.data[6], 0x00);
        assert_eq!(payload.data[7..11], 1.0f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_g8_ae_bass() {
        // Bass toggle: Format::AcousticEngine(0x18)
        let payload = create_payload(Format::AcousticEngine(0x18), 1.0);

        assert_eq!(payload.data[6], 0x18);
    }

    #[test]
    fn test_create_payload_g8_ae_bass_slider() {
        // Bass slider: Format::AcousticEngine(0x19)
        let payload = create_payload(Format::AcousticEngine(0x19), 0.5);

        assert_eq!(payload.data[6], 0x19);
        assert_eq!(payload.data[7..11], 0.5f32.to_le_bytes());
    }

    // G8 has no COMMIT for audio features
    #[test]
    fn test_create_payload_g8_ae_no_commit() {
        let payload = create_payload(Format::AcousticEngine(0x09), 1.0);

        // commit bytes should be all zeros (no COMMIT for G8 audio)
        assert_eq!(payload.commit[2..], [0u8; 63]);
    }

    // ============================================================
    // create_payload tests for G8 Crystal Voice
    // ============================================================

    #[test]
    fn test_create_payload_g8_cv_aec() {
        // AEC: Format::CrystalVoice(0x00)
        let payload = create_payload(Format::CrystalVoice(0x00), 1.0);

        assert_eq!(payload.data[2], 0x12);
        assert_eq!(payload.data[5], 0x95); // feature 0x0195
        assert_eq!(payload.data[6], 0x00); // sub-ID
        assert_eq!(payload.data[7..11], 1.0f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_g8_cv_noise_reduction() {
        // Noise Reduction: Format::CrystalVoice(0x04)
        let payload = create_payload(Format::CrystalVoice(0x04), 0.75);

        assert_eq!(payload.data[5], 0x95);
        assert_eq!(payload.data[6], 0x04);
        assert_eq!(payload.data[7..11], 0.75f32.to_le_bytes());
    }

    #[test]
    fn test_create_payload_g8_cv_voice_morph_toggle() {
        // Voice Morph toggle: Format::CrystalVoice(0x0a)
        let payload = create_payload(Format::CrystalVoice(0x0a), 1.0);

        assert_eq!(payload.data[5], 0x95);
        assert_eq!(payload.data[6], 0x0a);
    }

    #[test]
    fn test_create_payload_g8_cv_mic_eq() {
        // Mic EQ toggle: Format::CrystalVoice(0x13)
        let payload = create_payload(Format::CrystalVoice(0x13), 1.0);

        assert_eq!(payload.data[5], 0x95);
        assert_eq!(payload.data[6], 0x13);
    }

    // ============================================================
    // create_payload tests for G8 Decoder Options
    // ============================================================

    #[test]
    fn test_create_payload_g8_decoder_options() {
        // Decoder Options: Format::DecoderOptions(0x02)
        let payload = create_payload(Format::DecoderOptions(0x02), 1.0);

        assert_eq!(payload.data[2], 0x12);
        assert_eq!(payload.data[5], 0x97); // feature 0x0197
        assert_eq!(payload.data[6], 0x02); // sub-ID
        assert_eq!(payload.data[7..11], 1.0f32.to_le_bytes());
    }

    // ============================================================
    // create_payload tests for G8 Direct Mode
    // ============================================================

    #[test]
    fn test_create_payload_g8_direct_mode_on() {
        // Direct Mode ON: Format::DirectMode, value > 0
        let payload = create_payload(Format::DirectMode, 1.0);

        assert_eq!(payload.data[2], 0x39); // class
        assert_eq!(payload.data[3], 0x03); // length
        assert_eq!(payload.data[4], 0x05); // feature high
        assert_eq!(payload.data[5], 0x00); // feature low (0x0500)
        assert_eq!(payload.data[6], 0x01); // value ON
    }

    #[test]
    fn test_create_payload_g8_direct_mode_off() {
        // Direct Mode OFF: Format::DirectMode, value = 0
        let payload = create_payload(Format::DirectMode, 0.0);

        assert_eq!(payload.data[6], 0x00); // value OFF
    }

    // ============================================================
    // create_payload tests for G8 Energy Saving
    // ============================================================

    #[test]
    fn test_create_payload_g8_energy_saving_on() {
        // Energy Saving ON: Format::EnergySaving, value > 0
        let payload = create_payload(Format::EnergySaving, 1.0);

        assert_eq!(payload.data[2], 0x39); // class
        assert_eq!(payload.data[3], 0x03); // length
        assert_eq!(payload.data[4], 0x01); // feature high (0x0110)
        assert_eq!(payload.data[5], 0x10); // feature low
        assert_eq!(payload.data[6], 0x01); // value ON
    }

    #[test]
    fn test_create_payload_g8_energy_saving_off() {
        // Energy Saving OFF: Format::EnergySaving, value = 0
        let payload = create_payload(Format::EnergySaving, 0.0);

        assert_eq!(payload.data[6], 0x00); // value OFF
    }

    // ============================================================
    // create_payload tests for G6X RGB (not implemented)
    // ============================================================

    #[test]
    #[ignore] // RGB is not implemented yet
    fn test_create_payload_rgb() {
        let payload = create_payload(Format::RGB(0x01), 1.0);
        // This test is ignored until RGB is implemented
        let _ = payload;
    }

    // ============================================================
    // ISO_BANDS constant tests
    // ============================================================

    #[test]
    fn test_iso_bands_length() {
        assert_eq!(ISO_BANDS.len(), 10);
    }

    #[test]
    fn test_iso_bands_values() {
        assert_eq!(ISO_BANDS[0], 31.0);
        assert_eq!(ISO_BANDS[1], 62.0);
        assert_eq!(ISO_BANDS[2], 125.0);
        assert_eq!(ISO_BANDS[3], 250.0);
        assert_eq!(ISO_BANDS[4], 500.0);
        assert_eq!(ISO_BANDS[5], 1000.0);
        assert_eq!(ISO_BANDS[6], 2000.0);
        assert_eq!(ISO_BANDS[7], 4000.0);
        assert_eq!(ISO_BANDS[8], 8000.0);
        assert_eq!(ISO_BANDS[9], 16000.0);
    }

    // ============================================================
    // FEATURES constant tests
    // ============================================================

    #[test]
    fn test_features_not_empty() {
        assert!(!FEATURES.is_empty());
    }

    #[test]
    fn test_features_has_sbx() {
        let has_sbx = FEATURES.iter().any(|f| f.name == "SBX");
        assert!(has_sbx);
    }

    #[test]
    fn test_features_has_scout_mode() {
        let has_scout = FEATURES.iter().any(|f| f.name == "Scout Mode");
        assert!(has_scout);
    }

    #[test]
    fn test_features_has_eq() {
        let has_eq = FEATURES.iter().any(|f| f.name == "Equalizer");
        assert!(has_eq);
    }

    #[test]
    fn test_features_has_output_mode() {
        let has_output = FEATURES.iter().any(|f| f.name == "Output Mode");
        assert!(has_output);
    }

    #[test]
    fn test_features_eq_bands_count() {
        // "EQ " matches "EQ Pre-Amp" + 10 EQ bands (31Hz through 16kHz)
        // Pre-Amp is also a slider in the AE domain
        let eq_features: Vec<_> = FEATURES
            .iter()
            .filter(|f| f.name.starts_with("EQ "))
            .collect();
        // 10 EQ bands (31Hz through 16kHz) + "EQ Pre-Amp"
        assert_eq!(eq_features.len(), 11);
    }

    #[test]
    fn test_features_has_bass() {
        let has_bass = FEATURES.iter().any(|f| f.name == "Bass");
        assert!(has_bass);
    }

    #[test]
    fn test_features_has_crystalizer() {
        let has_crystalizer = FEATURES.iter().any(|f| f.name == "Crystalizer");
        assert!(has_crystalizer);
    }

    // ============================================================
    // get_ten_band_eq tests
    // ============================================================

    #[test]
    fn test_get_ten_band_eq_returns_array() {
        // This test only works with a real device, so we test the structure
        // We can't actually call get_ten_band_eq without a device connection
        // but we can verify the ISO_BANDS constant matches expected values
        let expected = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
        assert_eq!(ISO_BANDS, expected);
    }

    // ============================================================
    // Feature ID encoding tests
    // ============================================================

    #[test]
    fn test_feature_ids_are_unique() {
        let mut ids = Vec::new();
        for feature in FEATURES {
            let id_str = format!("{:?}", feature.id);
            assert!(
                !ids.contains(&id_str),
                "Duplicate feature ID found: {}",
                id_str
            );
            ids.push(id_str);
        }
    }

    #[test]
    fn test_feature_names_are_unique() {
        let mut names = Vec::new();
        for feature in FEATURES {
            assert!(
                !names.contains(&feature.name),
                "Duplicate feature name found: {}",
                feature.name
            );
            names.push(feature.name);
        }
    }

    // ============================================================
    // G8_FEATURES constant tests
    // ============================================================

    #[test]
    fn test_g8_features_not_empty() {
        assert!(!G8_FEATURES.is_empty());
    }

    #[test]
    fn test_g8_features_has_scout_mode() {
        let has_scout = G8_FEATURES.iter().any(|f| f.name == "Scout Mode");
        assert!(has_scout);
    }

    #[test]
    fn test_g8_features_has_acoustic_engine_eq() {
        let has_eq = G8_FEATURES.iter().any(|f| f.name == "Equalizer");
        assert!(has_eq);
    }

    #[test]
    fn test_g8_features_has_acoustic_engine_preamp() {
        let has_preamp = G8_FEATURES.iter().any(|f| f.name == "EQ Pre-Amp");
        assert!(has_preamp);
    }

    #[test]
    fn test_g8_features_has_acoustic_engine_10band_eq() {
        // 10-band EQ: 31Hz, 62Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz
        let eq_bands: Vec<_> = G8_FEATURES
            .iter()
            .filter(|f| f.name.starts_with("EQ ") && f.name != "Equalizer" && f.name != "EQ Pre-Amp")
            .collect();
        assert_eq!(eq_bands.len(), 10, "G8 should have 10 AE EQ bands");
    }

    #[test]
    fn test_g8_features_has_crystal_voice() {
        let has_aec = G8_FEATURES.iter().any(|f| f.name == "AEC");
        let has_nr = G8_FEATURES.iter().any(|f| f.name == "Noise Reduction");
        let has_mic_eq = G8_FEATURES.iter().any(|f| f.name == "Mic EQ");
        let has_voice_morph = G8_FEATURES.iter().any(|f| f.name == "Voice Morph");
        assert!(has_aec, "G8 should have AEC");
        assert!(has_nr, "G8 should have Noise Reduction");
        assert!(has_mic_eq, "G8 should have Mic EQ");
        assert!(has_voice_morph, "G8 should have Voice Morph");
    }

    #[test]
    fn test_g8_features_has_mic_eq_10band() {
        // 10-band Mic EQ: 31Hz, 62Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz
        let mic_eq_bands: Vec<_> = G8_FEATURES
            .iter()
            .filter(|f| f.name.starts_with("Mic EQ ") && f.name != "Mic EQ")
            .collect();
        assert_eq!(mic_eq_bands.len(), 10, "G8 should have 10 Mic EQ bands");
    }

    #[test]
    fn test_g8_features_has_decoder_mode() {
        let has_decoder = G8_FEATURES.iter().any(|f| f.name == "Decoder Mode");
        assert!(has_decoder);
    }

    #[test]
    fn test_g8_features_has_direct_mode() {
        let has_direct = G8_FEATURES.iter().any(|f| f.name == "Direct Mode");
        assert!(has_direct);
    }

    #[test]
    fn test_g8_features_has_energy_saving() {
        let has_energy = G8_FEATURES.iter().any(|f| f.name == "Energy Saving");
        assert!(has_energy);
    }

    #[test]
    fn test_g8_features_has_output_mode() {
        let has_output = G8_FEATURES.iter().any(|f| f.name == "Output Mode");
        assert!(has_output);
    }

    #[test]
    fn test_g8_feature_ids_are_unique() {
        let mut ids = Vec::new();
        for feature in G8_FEATURES {
            let id_str = format!("{:?}", feature.id);
            assert!(
                !ids.contains(&id_str),
                "Duplicate G8 feature ID found: {}",
                id_str
            );
            ids.push(id_str);
        }
    }

    #[test]
    fn test_g8_feature_names_are_unique() {
        let mut names = Vec::new();
        for feature in G8_FEATURES {
            assert!(
                !names.contains(&feature.name),
                "Duplicate G8 feature name found: {}",
                feature.name
            );
            names.push(feature.name);
        }
    }

    #[test]
    fn test_g8_features_acoustic_engine_format() {
        // Acoustic Engine features should have Format::AcousticEngine variant
        let ae_features: Vec<_> = G8_FEATURES
            .iter()
            .filter(|f| matches!(f.id, Format::AcousticEngine(_)))
            .collect();
        assert!(!ae_features.is_empty(), "G8 should have AcousticEngine features");
    }

    #[test]
    fn test_g8_features_crystal_voice_format() {
        // Crystal Voice features should have Format::CrystalVoice variant
        let cv_features: Vec<_> = G8_FEATURES
            .iter()
            .filter(|f| matches!(f.id, Format::CrystalVoice(_)))
            .collect();
        assert!(!cv_features.is_empty(), "G8 should have CrystalVoice features");
    }

    #[test]
    fn test_g8_features_decoder_options_format() {
        // Decoder Options feature should have Format::DecoderOptions variant
        let decoder_features: Vec<_> = G8_FEATURES
            .iter()
            .filter(|f| matches!(f.id, Format::DecoderOptions(_)))
            .collect();
        assert!(!decoder_features.is_empty(), "G8 should have DecoderOptions features");
    }

    #[test]
    fn test_g8_features_direct_mode_format() {
        // Direct Mode should have Format::DirectMode variant
        let direct_features: Vec<_> = G8_FEATURES
            .iter()
            .filter(|f| matches!(f.id, Format::DirectMode))
            .collect();
        assert!(!direct_features.is_empty(), "G8 should have DirectMode feature");
    }

    #[test]
    fn test_g8_features_energy_saving_format() {
        // Energy Saving should have Format::EnergySaving variant
        let energy_features: Vec<_> = G8_FEATURES
            .iter()
            .filter(|f| matches!(f.id, Format::EnergySaving))
            .collect();
        assert!(!energy_features.is_empty(), "G8 should have EnergySaving feature");
    }

    // ============================================================
    // DeviceFamily tests
    // ============================================================

    #[test]
    fn test_device_family_features() {
        assert!(!DeviceFamily::G6.features().is_empty());
        assert!(!DeviceFamily::G8USB1.features().is_empty());
        // G8USB2 has empty features (no HID features)
        assert!(DeviceFamily::G8USB2.features().is_empty());
    }

    #[test]
    fn test_device_family_names() {
        assert_eq!(DeviceFamily::G6.name(), "Sound Blaster X G6/G6X");
        assert_eq!(DeviceFamily::G8USB1.name(), "Sound Blaster G8 (USB-1)");
        assert_eq!(DeviceFamily::G8USB2.name(), "Sound Blaster G8 (USB-2)");
    }
}