use zymatica_zspar::{Concept8D, InvariantSet, ParityOnlyFrame, Rs12_8, SystematicFrame};

#[test]
fn systematic_crc_rejects_wire_corruption() {
    let state = Concept8D::new(2, 5, 9, 14, 15, 3, 1, 12);
    let frame = SystematicFrame::create(99, state, &InvariantSet::default());
    let mut wire = frame.serialize();
    assert!(SystematicFrame::parse(&wire).is_some());
    wire[17] ^= 1;
    assert!(SystematicFrame::parse(&wire).is_none());
}

#[test]
fn four_erasure_recovery_is_exact() {
    let data = [2, 5, 9, 14, 15, 3, 1, 12];
    let original = Rs12_8::encode(&data).unwrap();
    let mut corrupted = original;
    let erasures = [0u8, 2u8, 5u8, 7u8];
    for &p in &erasures {
        corrupted[p as usize] = 0;
    }
    let decoded = Rs12_8::decode(corrupted, &erasures);
    assert!(decoded.success());
    assert_eq!(decoded.codeword, original);
}

#[test]
fn semantic_tag_rejects_wrong_prediction_outside_code_radius() {
    let authoritative = Concept8D::new(2, 5, 9, 14, 15, 3, 1, 12);
    let frame = ParityOnlyFrame::create(101, authoritative, &InvariantSet::default());
    let mut predicted = authoritative;
    predicted.domain ^= 1;
    predicted.operation ^= 2;
    predicted.strength ^= 4;
    let repaired = frame.repair_prediction(predicted, &InvariantSet::default(), 0);
    assert!(!(repaired.success() && repaired.state != authoritative));
}
