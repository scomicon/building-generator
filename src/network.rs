use std::sync::RwLock;

use mod_api::{PayloadId, Registrar, ToClient, ToServer};

use crate::model::{
    BiomeQueryRequest, BiomeQueryResponse, BuildRequest, BuildResponse, RedoRequest, RerollRequest,
    RoadConnectRequest, SetLangRequest, UndoRequest,
};
use crate::{client_ui, server_builder};

#[derive(Clone)]
pub struct PayloadChannels {
    pub to_server_build: ToServer<BuildRequest>,
    pub to_server_road: ToServer<RoadConnectRequest>,
    pub to_server_undo: ToServer<UndoRequest>,
    pub to_server_redo: ToServer<RedoRequest>,
    pub to_server_reroll: ToServer<RerollRequest>,
    pub to_server_lang: ToServer<SetLangRequest>,
    pub to_server_biome: ToServer<BiomeQueryRequest>,
    pub to_client_reply: ToClient<BuildResponse>,
    pub to_client_biome: ToClient<BiomeQueryResponse>,
}

static CHANNELS: RwLock<Option<PayloadChannels>> = RwLock::new(None);

pub fn register_channels(registrar: &mut dyn Registrar) {
    let mut payloads = registrar.payload_handlers();
    let mut reg = payloads.registrar("1").optional();

    let to_server_build = reg.play_to_server_typed::<BuildRequest>(
        PayloadId("building_generator:build".into()),
        |req, cx| server_builder::handle_build_payload(req, cx),
    );
    let to_server_road = reg.play_to_server_typed::<RoadConnectRequest>(
        PayloadId("building_generator:connect_road".into()),
        |req, cx| server_builder::handle_road_payload(req, cx),
    );
    let to_server_undo = reg.play_to_server_typed::<UndoRequest>(
        PayloadId("building_generator:undo".into()),
        |req, cx| server_builder::handle_undo_payload(req, cx),
    );
    let to_server_redo = reg.play_to_server_typed::<RedoRequest>(
        PayloadId("building_generator:redo".into()),
        |req, cx| server_builder::handle_redo_payload(req, cx),
    );
    let to_server_reroll = reg.play_to_server_typed::<RerollRequest>(
        PayloadId("building_generator:reroll".into()),
        |req, cx| server_builder::handle_reroll_payload(req, cx),
    );
    let to_server_lang = reg.play_to_server_typed::<SetLangRequest>(
        PayloadId("building_generator:lang".into()),
        |req, cx| server_builder::handle_lang_payload(req, cx),
    );
    let to_server_biome = reg.play_to_server_typed::<BiomeQueryRequest>(
        PayloadId("building_generator:biome_query".into()),
        |req, cx| server_builder::handle_biome_payload(req, cx),
    );
    let to_client_reply = reg.play_to_client_typed::<BuildResponse>(
        PayloadId("building_generator:reply".into()),
        |resp, _| {
            client_ui::on_server_response(resp);
            Ok(())
        },
    );
    let to_client_biome = reg.play_to_client_typed::<BiomeQueryResponse>(
        PayloadId("building_generator:biome_reply".into()),
        |resp, _| {
            client_ui::on_biome_response(resp);
            Ok(())
        },
    );

    if let Ok(mut guard) = CHANNELS.write() {
        *guard = Some(PayloadChannels {
            to_server_build,
            to_server_road,
            to_server_undo,
            to_server_redo,
            to_server_reroll,
            to_server_lang,
            to_server_biome,
            to_client_reply,
            to_client_biome,
        });
    }
}

pub fn channels() -> Option<PayloadChannels> {
    CHANNELS.read().ok().and_then(|g| g.clone())
}
