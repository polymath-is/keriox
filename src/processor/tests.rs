use super::EventProcessor;
use crate::event_message::{parse, parse::Deserialized};
use crate::{database::lmdb::LmdbEventDatabase, database::EventDatabase, error::Error};
use std::fs;

#[test]
fn test_process() -> Result<(), Error> {
    use tempfile::Builder;

    // Create test db and event processor.
    let root = Builder::new().prefix("test-db").tempdir().unwrap();
    fs::create_dir_all(root.path()).unwrap();

    let db = LmdbEventDatabase::new(root.path()).unwrap();
    let event_processor = EventProcessor::new(db);

    let icp_raw = r#"{"vs":"KERI10JSON000159_","pre":"EUEtw_3JqBhrLtwwlP9QLnDXZGjJ3CIxq7QGP_dEQiwc","sn":"0","ilk":"icp","sith":"2","keys":["DSuhyBcPZEZLK-fcw5tzHn2N46wRCG_ZOoeKtWTOunRA","DVcuJOOJF1IE8svqEtrSuyQjGTd2HhfAkt9y2QkUtFJI","DT1iAhBWCkvChxNWsby2J0pJyxBIxbAtbLA0Ljx-Grh8"],"nxt":"E9izzBkXX76sqt0N-tfLzJeRqj0W56p4pDQ_ZqNCDpyw","toad":"0","wits":[],"cnfg":[]}-AADAAh_trqX993WCZfZ2Mm8Rj2AnlpJzStsv2x4M3gKOIpA740SCYGTDIU4L2Zokd8Krfakt98vy2vAYTjrJ7_UMnBQABNgYwwmeinupnrK8nIbVkz4iL7OgjAalNSNimZciYLCBRoKD5jbyXbHDxgycjl2vaw3roAzuaSi4686OY4P0kDgACbz0tl-U_EBbSfdKmtJHkSNfbDjB7pw_k9C9MuTv1eP3XM4OFApLJyhshWDtWmUzb4uorpXWvqRKfkMIRCKyBDQ"#;
    // Create deserialized inception event from string.
    // Events and sigs are from keripy `test_multisig_digprefix` test.
    let deserialized_icp = parse::signed_message(icp_raw.as_bytes()).unwrap().1;

    let (id, raw_parsed) = match &deserialized_icp {
        Deserialized::Event(e) => (e.event.event.event.prefix.clone(), e.event.raw.to_vec()),
        _ => Err(Error::SemanticError("bad deser".into()))?,
    };

    // Process icp event.
    let id_state = event_processor.process(deserialized_icp)?.unwrap();

    assert_eq!(id_state.sn, 0);
    // Check if processed event is in kel.
    let icp_from_db = event_processor.db.last_event_at_sn(&id, 0).unwrap();
    assert_eq!(icp_from_db, Some(raw_parsed));

    let rot_raw = r#"{"vs":"KERI10JSON000198_","pre":"EUEtw_3JqBhrLtwwlP9QLnDXZGjJ3CIxq7QGP_dEQiwc","sn":"1","ilk":"rot","dig":"EYmBZ0_Nn4sjid4UcQckAq_IXE6yzyh0Yy-lwKeRUVxg","sith":"2","keys":["DKPE5eeJRzkRTMOoRGVd2m18o8fLqM2j9kaxLhV3x8AQ","D1kcBE7h0ImWW6_Sp7MQxGYSshZZz6XM7OiUE5DXm0dU","D4JDgo3WNSUpt-NG14Ni31_GCmrU0r38yo7kgDuyGkQM"],"nxt":"EQpRYqbID2rW8X5lB6mOzDckJEIFae6NbJISXgJSN9qg","toad":"0","cuts":[],"adds":[],"data":[]}-AADAAtjBE4-kz5byJJDJuqKKKyjujw0CBMJfdx4XPmky_7cl8jNyeoTpcSbcifr7LUbuM_iQIBXFNIBqL9KMw8RQgAQABB8zTUrCwrBzO4M58oJ_CRu6fdVXK-jy5tYSwoqWcxjtRYnF-OIZ03zVjdhiky24-P_dRCGBQE-VmOQcSRW6NAgACrt7M9UM2Thvib1OhFcQtGjNnDNkG502_YWUnhOYOiS-_poEQRHi2PrF5FSNSv8cnAKgTH9UNt8h98kqOqXYJCQ"#;
    // Create deserialized rotation event.
    let deserialized_rot = parse::signed_message(rot_raw.as_bytes()).unwrap().1;

    let raw_parsed = match &deserialized_rot {
        Deserialized::Event(e) => e.event.raw.to_vec(),
        _ => Err(Error::SemanticError("bad deser".into()))?,
    };

    // Process rotation event.
    let id_state = event_processor.process(deserialized_rot.clone())?.unwrap();
    assert_eq!(id_state.sn, 1);

    // Check if processed event is in db.
    let rot_from_db = event_processor.db.last_event_at_sn(&id, 1).unwrap();
    assert_eq!(rot_from_db, Some(raw_parsed));

    // Process the same rotation event one more time.
    let id_state = event_processor.process(deserialized_rot);
    assert!(id_state.is_err());
    assert!(matches!(id_state, Err(Error::EventDuplicateError)));

    let ixn_raw = r#"{"vs":"KERI10JSON0000a3_","pre":"EUEtw_3JqBhrLtwwlP9QLnDXZGjJ3CIxq7QGP_dEQiwc","sn":"2","ilk":"ixn","dig":"EkH8Pm-Fv6QDawC4rDulf6X9anQ_AETbNdUh4HCjB0Co","data":[]}-AADAAYbN7F_JmSY9dZ5QzaccH8uaO6iCARwgebv4aw-MmM69Cn6iDWncWoK_Deu-Ik3hMTPpyhkUPsh444-psVFrhCAAB_YnGFnNbwJPiO1__3ecxOxFLBgvoAmSJ3j6ojA_a6tTbp19x0hg38OFvDlytbkbAXBCQPGrLDKoTclhFZ5guAQACpVhXP2WGe_Gd2aVpStB1NdRo9ipFFto4jyMeMWorUdCMMMwwTuIBa_gw62f4OyDTfWv4kSZo47l2li2RT6ydAw"#;
    // Create deserialized interaction event.
    let deserialized_ixn = parse::signed_message(ixn_raw.as_bytes()).unwrap().1;

    let raw_parsed = match &deserialized_ixn {
        Deserialized::Event(e) => e.event.raw.to_vec(),
        _ => Err(Error::SemanticError("bad deser".into()))?,
    };

    // Process interaction event.
    let id_state = event_processor.process(deserialized_ixn)?.unwrap();
    assert_eq!(id_state.sn, 2);

    // Check if processed event is in db.
    let ixn_from_db = event_processor.db.last_event_at_sn(&id, 2).unwrap();
    assert_eq!(ixn_from_db, Some(raw_parsed));

    // Construct partially signed interaction event.
    let ixn_raw = r#"{"vs":"KERI10JSON0000a3_","pre":"EUEtw_3JqBhrLtwwlP9QLnDXZGjJ3CIxq7QGP_dEQiwc","sn":"3","ilk":"ixn","dig":"EI8Y-mZzPFiY-RF7Pzvk11TP70op_xmX_8_X4ja01yPM","data":[]}-AADAAzyIUY_RJ_eXuPBor1a7bbiInTBntqMJLbzDzsTAfIHc3HB7SJThLKh2Oozkm38LIBrJF2xMXx5jjM70EQNZ4CgABNy-Ct5NW7W6W0347Uw8PMrQYpNVTT3DfgsfXMva2iVnYLzw9mQedhGILf1dsW2LIk5bvoQYBCCsVf6N16j-xAgACDaYuZa_09xZFgotKblT2BPuMETl9b73y6R7-LEe9jAE47RUAWeOFp6654Du1zB78UnM2jjKMrqMhG_q0BaD4Ag"#;
    let deserialized_ixn = parse::signed_message(ixn_raw.as_bytes()).unwrap().1;
    // Make event partially signed.
    let partially_signed_deserialized_ixn = match deserialized_ixn {
        Deserialized::Event(mut e) => {
            let sigs = e.signatures[1].clone();
            e.signatures = vec![sigs];
            Deserialized::Event(e)
        }
        _ => Err(Error::SemanticError("bad deser".into()))?,
    };

    // Process partially signed interaction event.
    let id_state = event_processor.process(partially_signed_deserialized_ixn);
    assert!(matches!(id_state, Err(Error::NotEnoughSigsError)));

    // Check if processed ixn event is in kel. It shouldn't because of not enough signatures.
    let ixn_from_db = event_processor.db.last_event_at_sn(&id, 3);
    assert!(matches!(ixn_from_db, Ok(None)));

    // Out of order event.
    let out_of_order_ixn_raw = r#"{"vs":"KERI10JSON0000a3_","pre":"EUEtw_3JqBhrLtwwlP9QLnDXZGjJ3CIxq7QGP_dEQiwc","sn":"4","ilk":"ixn","dig":"EI8Y-mZzPFiY-RF7Pzvk11TP70op_xmX_8_X4ja01yPM","data":[]}-AADAAzyIUY_RJ_eXuPBor1a7bbiInTBntqMJLbzDzsTAfIHc3HB7SJThLKh2Oozkm38LIBrJF2xMXx5jjM70EQNZ4CgABNy-Ct5NW7W6W0347Uw8PMrQYpNVTT3DfgsfXMva2iVnYLzw9mQedhGILf1dsW2LIk5bvoQYBCCsVf6N16j-xAgACDaYuZa_09xZFgotKblT2BPuMETl9b73y6R7-LEe9jAE47RUAWeOFp6654Du1zB78UnM2jjKMrqMhG_q0BaD4Ag"#;

    let out_of_order_ixn = parse::signed_message(out_of_order_ixn_raw.as_bytes())
        .unwrap()
        .1;

    let id_state = event_processor.process(out_of_order_ixn);
    assert!(id_state.is_err());
    assert!(matches!(id_state, Err(Error::EventOutOfOrderError)));

    // Check if processed event is in kel. It shouldn't.
    let ixn_from_db = event_processor.db.last_event_at_sn(&id, 4);
    assert!(matches!(ixn_from_db, Ok(None)));

    Ok(())
}

#[test]
fn test_process_receipt() -> Result<(), Error> {
    use tempfile::Builder;

    // Create test db and event processor.
    let root = Builder::new().prefix("test-db").tempdir().unwrap();
    fs::create_dir_all(root.path()).unwrap();

    let db = LmdbEventDatabase::new(root.path()).unwrap();
    let event_processor = EventProcessor::new(db);

    // Events and sigs are from keripy `test_direct_mode` test.
    // Construct and process controller's inception event.
    let icp_raw = r#"{"vs":"KERI10JSON0000fb_","pre":"EvEnZMhz52iTrJU8qKwtDxzmypyosgG70m6LIjkiCdoI","sn":"0","ilk":"icp","sith":"1","keys":["DSuhyBcPZEZLK-fcw5tzHn2N46wRCG_ZOoeKtWTOunRA"],"nxt":"EPYuj8mq_PYYsoBKkzX1kxSPGYBWaIya3slgCOyOtlqU","toad":"0","wits":[],"cnfg":[]}-AABAApYcYd1cppVg7Inh2YCslWKhUwh59TrPpIoqWxN2A38NCbTljvmBPBjSGIFDBNOvVjHpdZlty3Hgk6ilF8pVpAQ"#;
    let icp = parse::signed_message(icp_raw.as_bytes()).unwrap().1;

    let controller_id_state = event_processor.process(icp)?;

    // Construct receipt of controller's inception event.
    let vrc_raw = r#"{"vs":"KERI10JSON00010c_","pre":"EvEnZMhz52iTrJU8qKwtDxzmypyosgG70m6LIjkiCdoI","sn":"0","ilk":"vrc","dig":"EdpkS5j6xIAnPFjovQKLaou1jF7XcLny-pYZde4p35jc","seal":{"pre":"E0uTVILY2KXdcxX40MSM9Fr8EpGwfjMNap6ulAAzVt0M","dig":"Es0RthuviC_p-qHut_JCfMKSFwpljZ-WoppazqZIid-A"}}-AABAAcQJDHTzG8k1WYCR6LahLCIlcDED21Slz66piD9tcZo4VEmyWHYDccj4aRvVdy9xHqHsn38FMGN26x4S2skJGCw"#;
    let rcp = parse::signed_message(vrc_raw.as_bytes()).unwrap().1;

    let id_state = event_processor.process(rcp.clone());
    // Validator not yet in db. Event should be escrowed.
    assert!(id_state.is_err());

    // Contruct and process validator's inception event.
    let val_icp_raw = r#"{"vs":"KERI10JSON0000fb_","pre":"E0uTVILY2KXdcxX40MSM9Fr8EpGwfjMNap6ulAAzVt0M","sn":"0","ilk":"icp","sith":"1","keys":["D8KY1sKmgyjAiUDdUBPNPyrSz_ad_Qf9yzhDNZlEKiMc"],"nxt":"EOWDAJvex5dZzDxeHBANyaIoUG3F4-ic81G6GwtnC4f4","toad":"0","wits":[],"cnfg":[]}-AABAAR5dawnJxU_Gbb8EK2xUMLb2AU7wLlZDHlDzHvovP-YIowqFq719VMQc9hrEwW9JKs90leAm2rUp3_DOi7-olBg"#;
    let val_icp = parse::signed_message(val_icp_raw.as_bytes()).unwrap().1;

    event_processor.process(val_icp)?;

    // Process receipt once again.
    let id_state = event_processor.process(rcp);
    assert!(id_state.is_ok());
    // Controller's state shouldn't change after processing receipt.
    assert_eq!(controller_id_state, id_state?);

    Ok(())
}
