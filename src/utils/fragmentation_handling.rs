use bevy::audio::AudioSource;
use flate2::Compression;
use image::*;
use std::{io::Cursor, sync::Arc};
use wg_2024::{network::*, packet::*};

const STRINGBIT: u8 = 1;
const AUDIOBIT: u8 = 2;
const IMAGEBIT: u8 = 3;
const DEFAULTBIT: u8 = 4;
const MEDIABIT: u8 = 5;
const TEXTBIT: u8 = 6;
const CHATSTRINGBIT: u8 = 7;
const CHATIMAGEBIT: u8 = 8;
const CHATAUDIOBIT: u8 = 9;
const DEFRESPONSEBIT: u8 = 10;
const CONTENTRESPONSEBIT: u8 = 11;

trait MessageSend: Send + Sync + Sized {}
impl MessageSend for Message {}
impl MessageSend for DefaultsRequest {}
impl MessageSend for DefaultResponse {}
impl MessageSend for ChatMessages {}
impl MessageSend for ContentRequest {}
impl MessageSend for ContentResponse {}

#[derive(Clone, Debug)]
pub enum Message {
    DefaultsRequest(DefaultsRequest),
    DefaultResponse(DefaultResponse),
    ChatMessages(ChatMessages),
    ContentRequest(ContentRequest),
    ContentResponse(ContentResponse),
    String(String),
    Image(DynamicImage),
    Audio(AudioSource),
}

// Generic function for reconstructing a message
pub fn reconstruct_message(
    recognition_bit: u8,
    fragments: &mut Vec<Fragment>,
) -> Result<Message, &str> {
    match recognition_bit {
        1 => {
            if let Ok(res) = <String as Assembler<String>>::assemble(fragments) {
                Ok(Message::String(res))
            } else {
                Err("Failed to reconstruct String")
            }
        }
        2 => {
            if let Ok(res) = <AudioSource as Assembler<AudioSource>>::assemble(fragments) {
                Ok(Message::Audio(res))
            } else {
                Err("Failed to reconstruct Audio")
            }
        }
        3 => {
            if let Ok(res) = <DynamicImage as Assembler<DynamicImage>>::assemble(fragments) {
                Ok(Message::Image(res))
            } else {
                Err("Failed to reconstruct Image")
            }
        }
        4 => {
            if let Ok(res) = <DefaultsRequest as Assembler<DefaultsRequest>>::assemble(fragments) {
                Ok(Message::DefaultsRequest(res))
            } else {
                Err("Failed to reconstruct DefaultRequest")
            }
        }
        5 | 6 => {
            if let Ok(res) = <ContentRequest as Assembler<ContentRequest>>::assemble(fragments) {
                Ok(Message::ContentRequest(res))
            } else {
                Err("Failed to reconstruct ContentRequest")
            }
        }
        7 | 8 | 9 => {
            if let Ok(res) = <ChatMessages as Assembler<ChatMessages>>::assemble(fragments) {
                Ok(Message::ChatMessages(res))
            } else {
                Err("Failed to reconstruct ChatMessage")
            }
        }
        10 => {
            if let Ok(res) = <DefaultResponse as Assembler<DefaultResponse>>::assemble(fragments) {
                Ok(Message::DefaultResponse(res))
            } else {
                Err("Failed to reconstruct DefaultResponse")
            }
        }
        11 => {
            let cr = <ContentResponse as Assembler<ContentResponse>>::assemble(fragments);
            if let Ok(res) = cr {
                Ok(Message::ContentResponse(res))
            } else {
                Err("Failed to reconstruct ContentResponse")
            }
        }
        _ => Err("Bit recognition didn't produce any result"),
    }
}

pub fn deconstruct_message(msg: Message) -> Result<Vec<u8>, String> {
    match msg {
        Message::String(s) => Ok(<String as Fragmentation<String>>::fragment(s)),
        Message::Audio(track) => Ok(<AudioSource as Fragmentation<AudioSource>>::fragment(track)),
        Message::Image(img) => Ok(<DynamicImage as Fragmentation<DynamicImage>>::fragment(img)),
        Message::DefaultsRequest(df) => {
            Ok(<DefaultsRequest as Fragmentation<DefaultsRequest>>::fragment(df))
        }
        Message::ContentRequest(cr) => Ok(
            <ContentRequest as Fragmentation<ContentRequest>>::fragment(cr),
        ),
        Message::ChatMessages(cm) => {
            Ok(<ChatMessages as Fragmentation<ChatMessages>>::fragment(cm))
        }
        Message::DefaultResponse(df) => {
            Ok(<DefaultResponse as Fragmentation<DefaultResponse>>::fragment(df))
        }
        Message::ContentResponse(cr) => {
            Ok(<ContentResponse as Fragmentation<ContentResponse>>::fragment(cr))
        }
    }
}

// Trait to handle message fragmentation
//      Every impl Fragemntation has a diff recognition bit, that is the first element
//      of the vector of the message's bytes. It will be used then to be the first fragment
//      so that when reconstructing a message the types can be inferred.

pub trait Fragmentation<T> {
    fn fragment(message: T) -> Vec<u8>; // Fragment a message into bytes
}

// Helper function to sort fragments by their index
fn sort_by_fragment_index(fragments: &mut Vec<Fragment>) {
    let len = fragments.len();
    for i in 0..len {
        for j in 0..len {
            if fragments[i].fragment_index < fragments[j].fragment_index {
                let tmp = fragments[i].clone();
                fragments[i] = fragments[j].clone();
                fragments[j] = tmp;
            }
        }
    }
}

// Function to check if all fragments are present
fn check_wholeness(fragments: &mut Vec<Fragment>) -> bool {
    let size = fragments[0].total_n_fragments; // Total number of fragments
    let mut count = 0;
    for i in 1..size + 1 {
        count += i as u64; // Sum of expected fragment indices
    }
    let mut check_count = 0;
    for fr in fragments {
        check_count += fr.fragment_index; // Sum of actual fragment indices
    }
    check_count == count // Verify completeness
}

fn split_on_one(input: Vec<u8>) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut current_chunk = Vec::new();

    for &byte in &input {
        if byte == 1 {
            // If current_chunk is not empty, push it to the result
            if !current_chunk.is_empty() {
                result.push(current_chunk);
                current_chunk = Vec::new();
            }
        }
        // Always add the byte to the current chunk
        current_chunk.push(byte);
    }

    // Push the last chunk if it's not empty
    if !current_chunk.is_empty() {
        result.push(current_chunk);
    }

    result
}

// Trait to assemble fragments into the original message
pub trait Assembler<T: Fragmentation<T>> {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<T, String>;
}

// Implementation of Fragmentation for String
impl Fragmentation<String> for String {
    fn fragment(message: String) -> Vec<u8> {
        let mut vec = [STRINGBIT].to_vec();
        vec.append(&mut message.into_bytes()); // Convert the string into bytes
        vec
    }
}

// Implementation of Assembler for String
impl Assembler<String> for String {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<String, String> {
        sort_by_fragment_index(fragments); // Sort fragments
        if !check_wholeness(fragments) {
            return Err(
                "Missing one or more fragments. Cannot reconstruct the message".to_string(),
            );
        } else {
            let mut vec = Vec::new();
            for fr in fragments {
                if fr.fragment_index != 1 {
                    for i in 0..fr.length {
                        vec.push(fr.data[i as usize]); // Collect fragment data
                    }
                }
            }
            Ok(String::from_utf8(vec).expect("Something is wrong with the assembler"))
            // Reconstruct string
        }
    }
}

// Implementation of Fragmentation for Bevy's AudioSource
impl Fragmentation<bevy::audio::AudioSource> for AudioSource {
    fn fragment(message: bevy::audio::AudioSource) -> Vec<u8> {
        let mut vec = [AUDIOBIT].to_vec();
        let mut compressed = compress(&message.bytes);
        vec.append(&mut compressed); // Extract bytes from AudioSource
        vec
    }
}

// Implementation of Assembler for Bevy's AudioSource
impl Assembler<bevy::audio::AudioSource> for AudioSource {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<bevy::audio::AudioSource, String> {
        sort_by_fragment_index(fragments); // Sort fragments
        if !check_wholeness(fragments) {
            return Err(
                "Missing one or more fragments. Cannot reconstruct the message".to_string(),
            );
        } else {
            let mut vec = Vec::new();
            for fr in fragments {
                if fr.fragment_index != 1 {
                    for i in 0..fr.length {
                        vec.push(fr.data[i as usize]); // Collect fragment data
                    }
                }
            }
            let decompressed = decompress(vec.as_slice());
            Ok(AudioSource {
                bytes: Arc::from(decompressed),
            }) // Create new AudioSource
        }
    }
}

// Implementation of Fragmentatio for images(for now just png)
impl Fragmentation<image::DynamicImage> for image::DynamicImage {
    fn fragment(message: image::DynamicImage) -> Vec<u8> {
        let mut vec = [IMAGEBIT].to_vec();
        let mut data = Vec::new();
        message
            .write_to(&mut Cursor::new(&mut data), image::ImageFormat::Png)
            .unwrap(); // Extract data from Image
        let mut compressed = compress(data.as_slice());
        vec.append(&mut compressed);
        vec
    }
}

// Implementation of Assembler for Bevy's Image
impl Assembler<image::DynamicImage> for image::DynamicImage {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<image::DynamicImage, String> {
        // Sort fragments by index
        fragments.sort_by_key(|fr| fr.fragment_index);

        // Check if all fragments are present
        if !check_wholeness(fragments) {
            return Err(
                "Missing one or more fragments. Cannot reconstruct the message.".to_string(),
            );
        } else {
            // Combine data from fragments
            let mut combined_data = Vec::new();
            for fragment in fragments.iter() {
                if fragment.fragment_index != 1 {
                    combined_data.extend_from_slice(&fragment.data[..fragment.length as usize]);
                }
            }
            let decompressed = decompress(combined_data.as_slice());
            let reader =
                codecs::png::PngDecoder::new(Cursor::new(decompressed)).expect("Error in decoder");
            let res = image::DynamicImage::from_decoder(reader);
            // Decode the image

            match res {
                Ok(image) => Ok(image),
                Err(_) => Err(res.err().unwrap().to_string()),
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DefaultsRequest {
    REGISTER,         //client register to chat server
    GETALLTEXT,       //request all text file inside of content server
    GETALLMEDIALINKS, //request all media links insede of content server
    GETALLAVAILABLE,  //get all client available for chatting
    GETSERVERTYPE,    //get servertype
}

impl Fragmentation<DefaultsRequest> for DefaultsRequest {
    fn fragment(message: DefaultsRequest) -> Vec<u8> {
        match message {
            DefaultsRequest::REGISTER => {
                vec![DEFAULTBIT, 1]
            }
            DefaultsRequest::GETALLTEXT => {
                vec![DEFAULTBIT, 2]
            }
            DefaultsRequest::GETALLMEDIALINKS => {
                vec![DEFAULTBIT, 3]
            }
            DefaultsRequest::GETALLAVAILABLE => {
                vec![DEFAULTBIT, 4]
            }
            DefaultsRequest::GETSERVERTYPE => {
                vec![DEFAULTBIT, 5]
            }
        }
    }
}

impl Assembler<DefaultsRequest> for DefaultsRequest {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<DefaultsRequest, String> {
        if fragments.len() != 2 {
            Err("Lenght of default requests must be 2".to_string())
        } else {
            //match the second fragment first bit for recognition.
            match fragments[1].data[0] {
                1 => Ok(DefaultsRequest::REGISTER),
                2 => Ok(DefaultsRequest::GETALLTEXT),
                3 => Ok(DefaultsRequest::GETALLMEDIALINKS),
                4 => Ok(DefaultsRequest::GETALLAVAILABLE),
                5 => Ok(DefaultsRequest::GETSERVERTYPE),
                _ => Err("Default request identifier does not match".to_string()),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentRequest {
    GETTEXT(String), //get specific text file, String is the path inside the assets directory
    GETMEDIA(String), //get specific media, String is the path inside of the assets directory
}

impl ContentRequest {
    pub fn new_get_text_req(path: String) -> Self {
        ContentRequest::GETTEXT(path)
    }

    pub fn new_get_media_req(path: String) -> Self {
        ContentRequest::GETMEDIA(path)
    }
}

impl Fragmentation<ContentRequest> for ContentRequest {
    fn fragment(message: ContentRequest) -> Vec<u8> {
        match message {
            ContentRequest::GETMEDIA(path) => {
                let mut vec = vec![MEDIABIT];
                vec.append(
                    &mut <String as Fragmentation<String>>::fragment(path)
                        .split_at(1)
                        .1
                        .to_vec(),
                );
                vec
            }
            ContentRequest::GETTEXT(path) => {
                let mut vec = vec![TEXTBIT];
                vec.append(
                    &mut <String as Fragmentation<String>>::fragment(path)
                        .split_at(1)
                        .1
                        .to_vec(),
                );
                vec
            }
        }
    }
}

impl Assembler<ContentRequest> for ContentRequest {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<ContentRequest, String> {
        match fragments[0].data[0] {
            MEDIABIT => {
                let mut vec = fragments.split_at(1).1.to_vec();
                let frag_1 = Fragment::new(1, fragments.len() as u64, slice_to_array(&[1], 1));
                let mut vec1 = vec![frag_1];
                vec1.append(&mut vec);
                if let Ok(str) = <String as Assembler<String>>::assemble(&mut vec1) {
                    Ok(ContentRequest::GETMEDIA(str))
                } else {
                    Err("Error assembling the message".to_string())
                }
            }
            TEXTBIT => {
                let mut vec = fragments.split_at(1).1.to_vec();
                let frag_1 = Fragment::new(1, fragments.len() as u64, slice_to_array(&[1], 1));
                let mut vec1 = vec![frag_1];
                vec1.append(&mut vec);
                if let Ok(str) = <String as Assembler<String>>::assemble(&mut vec1) {
                    Ok(ContentRequest::GETTEXT(str))
                } else {
                    Err("Error assembling the message".to_string())
                }
            }
            _ => Err("No match for ContentRequst".to_string()),
        }
    }
}

#[derive(Debug, Clone)]

pub enum ChatMessages {
    CHATSTRING(NodeId, NodeId, NodeId, String), //send to specific client to simulate chat behaviour
    CHATIMAGE(NodeId, NodeId, NodeId, DynamicImage),
    CHATAUDIO(NodeId, NodeId, NodeId, AudioSource),
}

impl PartialEq for ChatMessages {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ChatMessages::CHATSTRING(a, b, c, d), ChatMessages::CHATSTRING(a1, b1, c1, d1)) => {
                a == a1
                    && b == b1
                    && c == c1
                    && <String as Fragmentation<String>>::fragment(d.clone())
                        == <String as Fragmentation<String>>::fragment(d1.clone())
            }
            (ChatMessages::CHATAUDIO(a, b, c, d), ChatMessages::CHATAUDIO(a1, b1, c1, d1)) => {
                a == a1
                    && b == b1
                    && c == c1
                    && <AudioSource as Fragmentation<AudioSource>>::fragment(d.clone())
                        == <AudioSource as Fragmentation<AudioSource>>::fragment(d1.clone())
            }
            (ChatMessages::CHATIMAGE(a, b, c, d), ChatMessages::CHATIMAGE(a1, b1, c1, d1)) => {
                a == a1
                    && b == b1
                    && c == c1
                    && <DynamicImage as Fragmentation<DynamicImage>>::fragment(d.clone())
                        == <DynamicImage as Fragmentation<DynamicImage>>::fragment(d1.clone())
            }
            _ => false,
        }
    }
}

impl Eq for ChatMessages {}

impl ChatMessages {
    pub fn new_string_msg(src: NodeId, srv: NodeId, dst: NodeId, msg: String) -> Self {
        ChatMessages::CHATSTRING(src, srv, dst, msg)
    }

    pub fn new_image_msg(src: NodeId, srv: NodeId, dst: NodeId, img: DynamicImage) -> Self {
        ChatMessages::CHATIMAGE(src, srv, dst, img)
    }

    pub fn new_audio_msg(src: NodeId, srv: NodeId, dst: NodeId, track: AudioSource) -> Self {
        ChatMessages::CHATAUDIO(src, srv, dst, track)
    }
}

impl Fragmentation<ChatMessages> for ChatMessages {
    fn fragment(message: ChatMessages) -> Vec<u8> {
        match message {
            ChatMessages::CHATSTRING(src, srv, dst, msg) => {
                let mut vec = [CHATSTRINGBIT].to_vec();
                vec.push(src);
                vec.push(srv);
                vec.push(dst);
                vec.append(&mut <String as Fragmentation<String>>::fragment(msg));
                vec
            }
            ChatMessages::CHATIMAGE(src, srv, dst, msg) => {
                let mut vec = [CHATIMAGEBIT].to_vec();
                vec.push(src);
                vec.push(srv);
                vec.push(dst);
                vec.append(&mut <DynamicImage as Fragmentation<DynamicImage>>::fragment(msg));
                vec
            }
            ChatMessages::CHATAUDIO(src, srv, dst, msg) => {
                let mut vec = [CHATAUDIOBIT].to_vec();
                vec.push(src);
                vec.push(srv);
                vec.push(dst);
                vec.append(&mut <AudioSource as Fragmentation<AudioSource>>::fragment(
                    msg,
                ));
                vec
            }
        }
    }
}

impl Assembler<ChatMessages> for ChatMessages {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<ChatMessages, String> {
        let src = fragments[1].data[0];
        let srv = fragments[1].data[1];
        let dst = fragments[1].data[2];
        let recognition_bit = fragments[1].data[3];
        let mut bytes = Vec::new();
        for frag in fragments.clone() {
            if frag.fragment_index > 2 {
                if frag.length < 128 {
                    bytes.append(&mut frag.data.split_at(frag.length as usize).0.to_vec());
                } else {
                    bytes.append(&mut frag.data.to_vec());
                }
            }
            if frag.fragment_index == 2 {
                if frag.length < 128 {
                    bytes.append(
                        &mut frag
                            .data
                            .split_at(frag.length as usize)
                            .0
                            .split_at(3)
                            .1
                            .to_vec(),
                    );
                } else {
                    bytes.append(&mut frag.data.split_at(3).1.to_vec());
                }
            }
        }
        match fragments[0].data[0] {
            CHATIMAGEBIT => {
                if recognition_bit == IMAGEBIT {
                    let mut series = serialize(bytes);
                    let msg = <DynamicImage as Assembler<DynamicImage>>::assemble(&mut series);
                    if let Some(res) = msg.clone().ok() {
                        Ok(ChatMessages::CHATIMAGE(src, srv, dst, res))
                    } else {
                        Err(msg.err().unwrap_or(
                            "Something went wrong with the image reconstruction".to_string(),
                        ))
                    }
                } else {
                    Err("Recognition bits don't match".to_string())
                }
            }
            CHATSTRINGBIT => {
                if recognition_bit == STRINGBIT {
                    let mut series = serialize(bytes);
                    eprintln!("{} {}", series[0].total_n_fragments, series[1].length);
                    let msg = <String as Assembler<String>>::assemble(&mut series);
                    if let Some(res) = msg.clone().ok() {
                        Ok(ChatMessages::CHATSTRING(src, srv, dst, res))
                    } else {
                        Err(msg.err().unwrap_or(
                            "Something went wrong with the string reconstruction".to_string(),
                        ))
                    }
                } else {
                    Err("Recognition bits don't match".to_string())
                }
            }
            CHATAUDIOBIT => {
                if recognition_bit == AUDIOBIT {
                    let mut series = serialize(bytes);
                    let msg = <AudioSource as Assembler<AudioSource>>::assemble(&mut series);
                    if let Some(res) = msg.clone().ok() {
                        Ok(ChatMessages::CHATAUDIO(src, srv, dst, res))
                    } else {
                        Err(msg.err().unwrap_or(
                            "Something went wrong with the audio reconstruction".to_string(),
                        ))
                    }
                } else {
                    Err("Recognition bits don't match".to_string())
                }
            }
            _ => Err("Message not supported for chats".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DefaultResponse {
    REGISTERED(bool, NodeId),
    ALLTEXT(Vec<String>),
    ALLMEDIALINKS(Vec<String>),
    ALLAVAILABLE(Vec<NodeId>),
    SERVERTYPE(u8, NodeId), //1: textServer, 2: mediaServer, 3: chatServer
    ERRNOTEXT,
    ERRNOMEDIA,
    ERRNOAVAILABLE,
}

impl DefaultResponse {
    pub fn new_registered_rsp(val: bool, id: NodeId) -> Self {
        DefaultResponse::REGISTERED(val, id)
    }
    pub fn new_all_text_rsp(text_links: Vec<String>) -> Self {
        DefaultResponse::ALLTEXT(text_links)
    }
    pub fn new_all_media_rsp(media_links: Vec<String>) -> Self {
        DefaultResponse::ALLMEDIALINKS(media_links)
    }
    pub fn new_available_rsp(available_ids: Vec<NodeId>) -> Self {
        DefaultResponse::ALLAVAILABLE(available_ids)
    }
    pub fn new_server_type_rsp(id_type: u8, id: NodeId) -> Self {
        DefaultResponse::SERVERTYPE(id_type, id)
    }
    pub fn new_err_no_text_rsp() -> Self {
        DefaultResponse::ERRNOTEXT
    }
    pub fn new_err_no_media_rsp() -> Self {
        DefaultResponse::ERRNOMEDIA
    }
    pub fn new_no_available_rsp() -> Self {
        DefaultResponse::ERRNOAVAILABLE
    }
}

impl Fragmentation<DefaultResponse> for DefaultResponse {
    fn fragment(message: DefaultResponse) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.push(DEFRESPONSEBIT);

        match message {
            DefaultResponse::REGISTERED(val, id) => {
                let mut bit = 0;
                if val {
                    bit = 1;
                }
                let mut vec_1 = [0, bit, id].to_vec();
                vec.append(&mut vec_1);
                vec
            }
            DefaultResponse::ALLTEXT(links) => {
                vec.push(1);
                for l in links {
                    let mut bytes = <String as Fragmentation<String>>::fragment(l);
                    vec.append(&mut bytes);
                }
                vec
            }
            DefaultResponse::ALLMEDIALINKS(links) => {
                vec.push(2);
                for l in links {
                    let mut bytes = <String as Fragmentation<String>>::fragment(l);
                    vec.append(&mut bytes);
                }
                vec
            }
            DefaultResponse::ALLAVAILABLE(ids) => {
                vec.push(3);
                for id in ids {
                    vec.push(id);
                }
                vec
            }
            DefaultResponse::SERVERTYPE(typ, id) => {
                vec.append(&mut [4, typ, id].to_vec());
                vec
            }
            DefaultResponse::ERRNOTEXT => {
                vec.push(5);
                vec
            }
            DefaultResponse::ERRNOMEDIA => {
                vec.push(6);
                vec
            }
            DefaultResponse::ERRNOAVAILABLE => {
                vec.push(7);
                vec
            }
        }
    }
}

impl Assembler<DefaultResponse> for DefaultResponse {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<DefaultResponse, String> {
        if fragments[0].data[0] == DEFRESPONSEBIT {
            match fragments[1].data[0] {
                0 => {
                    if fragments[1].data[1] == 1 {
                        Ok(DefaultResponse::REGISTERED(true, fragments[1].data[2]))
                    } else {
                        Ok(DefaultResponse::REGISTERED(false, fragments[1].data[2]))
                    }
                }
                1 => {
                    let mut tmp = Vec::new();
                    for frag in fragments {
                        if frag.fragment_index > 2 {
                            if frag.length < 128 {
                                tmp.append(
                                    &mut frag.data.split_at(frag.length as usize).0.to_vec(),
                                );
                            } else {
                                tmp.append(&mut frag.data.to_vec());
                            }
                        }
                        if frag.fragment_index == 2 {
                            if frag.length < 128 {
                                let data = frag.data.split_at(frag.length as usize).0.to_vec();
                                tmp.append(&mut data.split_at(1).1.to_vec());
                            } else {
                                tmp.append(&mut frag.data.split_at(1).1.to_vec());
                            }
                        }
                    }
                    let vecs = split_on_one(tmp);
                    let mut vec = Vec::new();
                    for bytes in vecs {
                        let mut ser = serialize(bytes);
                        let str = <String as Assembler<String>>::assemble(&mut ser);
                        if let Some(res) = str.ok() {
                            vec.push(res);
                        }
                    }
                    Ok(DefaultResponse::ALLTEXT(vec))
                }
                2 => {
                    let mut tmp = Vec::new();
                    for frag in fragments {
                        if frag.fragment_index > 2 {
                            if frag.length < 128 {
                                tmp.append(
                                    &mut frag.data.split_at(frag.length as usize).0.to_vec(),
                                );
                            } else {
                                tmp.append(&mut frag.data.to_vec());
                            }
                        }
                        if frag.fragment_index == 2 {
                            if frag.length < 128 {
                                let data = frag.data.split_at(frag.length as usize).0.to_vec();
                                tmp.append(&mut data.split_at(1).1.to_vec());
                            } else {
                                tmp.append(&mut frag.data.split_at(1).1.to_vec());
                            }
                        }
                    }
                    let vecs = split_on_one(tmp);
                    let mut vec = Vec::new();
                    for bytes in vecs {
                        let mut ser = serialize(bytes);
                        let str = <String as Assembler<String>>::assemble(&mut ser);
                        if let Some(res) = str.ok() {
                            vec.push(res);
                        }
                    }
                    Ok(DefaultResponse::ALLMEDIALINKS(vec))
                }
                3 => {
                    let mut tmp = Vec::new();
                    for frag in fragments {
                        if frag.fragment_index > 2 {
                            if frag.length < 128 {
                                tmp.append(
                                    &mut frag.data.split_at(frag.length as usize).0.to_vec(),
                                );
                            } else {
                                tmp.append(&mut frag.data.to_vec());
                            }
                        }
                        if frag.fragment_index == 2 {
                            if frag.length < 128 {
                                let data = frag.data.split_at(frag.length as usize).0.to_vec();
                                tmp.append(&mut data.split_at(1).1.to_vec());
                            } else {
                                tmp.append(&mut frag.data.split_at(1).1.to_vec());
                            }
                        }
                    }
                    Ok(DefaultResponse::ALLAVAILABLE(tmp))
                }
                4 => {
                    if fragments[1].data[1] >= 1 && fragments[1].data[1] <= 3 {
                        Ok(DefaultResponse::SERVERTYPE(
                            fragments[1].data[1],
                            fragments[1].data[2],
                        ))
                    } else {
                        Err("Error in getting the type".to_string())
                    }
                }
                5 => Ok(DefaultResponse::ERRNOTEXT),
                6 => Ok(DefaultResponse::ERRNOMEDIA),
                7 => Ok(DefaultResponse::ERRNOAVAILABLE),
                _ => Err("Error when reconstructing message".to_string()),
            }
        } else {
            Err("Wrong bit for Default Response Reconstruction".to_string())
        }
    }
}

#[derive(Debug, Clone)]
pub enum ContentResponse {
    TEXT(Vec<String>),
    MEDIAIMAGE(DynamicImage),
    MEDIAUDIO(AudioSource),
    NOTEXTFOUND,
    NOMEDIAFOUND,
}

impl ContentResponse {
    pub fn new_text_resp(text: Vec<String>) -> Self {
        Self::TEXT(text)
    }

    pub fn new_img_resp(img: DynamicImage) -> Self {
        Self::MEDIAIMAGE(img)
    }

    pub fn new_audio_resp(track: AudioSource) -> Self {
        Self::MEDIAUDIO(track)
    }

    pub fn new_no_text_found_resp() -> Self {
        Self::NOTEXTFOUND
    }

    pub fn new_no_media_found_resp() -> Self {
        Self::NOMEDIAFOUND
    }
}

impl Fragmentation<ContentResponse> for ContentResponse {
    fn fragment(message: ContentResponse) -> Vec<u8> {
        let mut vec = [CONTENTRESPONSEBIT].to_vec();
        match message {
            ContentResponse::TEXT(string) => {
                vec.push(0);
                for s in string {
                    let mut bytes = <String as Fragmentation<String>>::fragment(s);
                    vec.append(&mut bytes);
                }
                vec
            }
            ContentResponse::MEDIAIMAGE(img) => {
                vec.push(1);
                let mut bytes = <DynamicImage as Fragmentation<DynamicImage>>::fragment(img);
                vec.append(&mut bytes);
                vec
            }
            ContentResponse::MEDIAUDIO(src) => {
                vec.push(2);
                let mut bytes = <AudioSource as Fragmentation<AudioSource>>::fragment(src);
                vec.append(&mut bytes);
                vec
            }
            ContentResponse::NOTEXTFOUND => {
                vec.push(3);
                vec
            }
            ContentResponse::NOMEDIAFOUND => {
                vec.push(4);
                vec
            }
        }
    }
}

impl Assembler<ContentResponse> for ContentResponse {
    fn assemble(fragments: &mut Vec<Fragment>) -> Result<ContentResponse, String> {
        if fragments[0].data[0] == CONTENTRESPONSEBIT {
            match fragments[1].data[0] {
                0 => {
                    let mut tmp = Vec::new();
                    for frag in fragments {
                        if frag.fragment_index > 2 {
                            if frag.length < 128 {
                                tmp.append(
                                    &mut frag.data.split_at(frag.length as usize).0.to_vec(),
                                );
                            } else {
                                tmp.append(&mut frag.data.to_vec());
                            }
                        }
                        if frag.fragment_index == 2 {
                            if frag.length < 128 {
                                let mut data = frag.data.split_at(frag.length as usize).0.to_vec();
                                if data.len() > 1 {
                                    tmp.append(&mut data.split_at(1).1.to_vec());
                                } else {
                                    tmp.append(&mut data);
                                }
                            } else {
                                tmp.append(&mut frag.data.split_at(1).1.to_vec());
                            }
                        }
                    }
                    let vecs = split_on_one(tmp);
                    let mut vec = Vec::new();
                    for bytes in vecs {
                        let mut ser = serialize(bytes);
                        let str = <String as Assembler<String>>::assemble(&mut ser);
                        if let Some(res) = str.ok() {
                            vec.push(res);
                        }
                    }
                    Ok(ContentResponse::TEXT(vec))
                }
                1 => {
                    if fragments[1].data[1] == IMAGEBIT {
                        let mut tmp = Vec::new();
                        for frag in fragments {
                            if frag.fragment_index == 2 {
                                if frag.length < 128 {
                                    let data = frag.data.split_at(frag.length as usize).0.to_vec();
                                    tmp.append(&mut data.split_at(1).1.to_vec());
                                } else {
                                    tmp.append(&mut frag.data.split_at(1).1.to_vec());
                                }
                            }
                            if frag.fragment_index > 2 {
                                if frag.length < 128 {
                                    tmp.append(
                                        &mut frag.data.split_at(frag.length as usize).0.to_vec(),
                                    );
                                } else {
                                    tmp.append(&mut frag.data.to_vec());
                                }
                            }
                        }
                        let mut ser = serialize(tmp);
                        let img = <DynamicImage as Assembler<DynamicImage>>::assemble(&mut ser);
                        if let Some(res) = img.clone().ok() {
                            return Ok(ContentResponse::MEDIAIMAGE(res));
                        } else {
                            return Err(img.clone().err().unwrap());
                        }
                    } else {
                        Err("IMAGEBIT diffrent than the bit found".to_string())
                    }
                }
                2 => {
                    if fragments[1].data[1] == AUDIOBIT {
                        let mut tmp = Vec::new();
                        for frag in fragments {
                            if frag.fragment_index == 2 {
                                if frag.length < 128 {
                                    let data = frag.data.split_at(frag.length as usize).0.to_vec();
                                    tmp.append(&mut data.split_at(1).1.to_vec());
                                } else {
                                    tmp.append(&mut frag.data.split_at(1).1.to_vec());
                                }
                            }
                            if frag.fragment_index > 2 {
                                if frag.length < 128 {
                                    tmp.append(
                                        &mut frag.data.split_at(frag.length as usize).0.to_vec(),
                                    );
                                } else {
                                    tmp.append(&mut frag.data.to_vec());
                                }
                            }
                        }
                        let mut ser = serialize(tmp);
                        let src = <AudioSource as Assembler<AudioSource>>::assemble(&mut ser);
                        if let Some(res) = src.clone().ok() {
                            return Ok(ContentResponse::MEDIAUDIO(res));
                        } else {
                            return Err(src.clone().err().unwrap());
                        }
                    } else {
                        Err("AUDIOBIT diffrent than the bit found".to_string())
                    }
                }
                3 => Ok(ContentResponse::NOTEXTFOUND),
                4 => Ok(ContentResponse::NOMEDIAFOUND),
                _ => Err("No Appropriate Response Found".to_string()),
            }
        } else {
            Err("Wrong bit for ContentResponse reconstruction".to_string())
        }
    }
}

fn slice_to_array(slice: &[u8], len: usize) -> [u8; 128] {
    let mut res: [u8; 128] = [0; 128];
    for i in 0..len {
        res[i] = slice[i];
    }
    res
}

// Serialize data into fragments
pub fn serialize(datas: Vec<u8>) -> Vec<Fragment> {
    let (f0, data) = datas.split_at(1);
    let len = data.len();
    let mut iter = data.chunks(128); // Split data into chunks of 128 bytes
    let mut vec = Vec::new();
    let mut size = ((len / 128) + 1) as u64;
    let last = (len % 128) as u64;
    if last != 0 {
        size += 1; // Adjust total size for remaining data
    }

    let frag_0 = Fragment {
        fragment_index: 1,
        total_n_fragments: size,
        data: slice_to_array(f0, 1),
        length: 1,
    };
    vec.push(frag_0);
    let mut i = 2;
    let mut j = 128;
    if len > 128 {
        loop {
            if j < len {
                let fragment_data = iter.next().unwrap();
                vec.push(Fragment {
                    fragment_index: i,
                    total_n_fragments: size,
                    data: slice_to_array(fragment_data, fragment_data.len()),
                    length: fragment_data.len() as u8,
                });
                i += 1;
                j += 128;
            } else {
                let fragment_data = iter.next().unwrap();
                vec.push(Fragment {
                    fragment_index: i,
                    total_n_fragments: size,
                    data: slice_to_array(fragment_data, fragment_data.len()),
                    length: fragment_data.len() as u8,
                });
                break;
            }
        }
    } else {
        vec.push(Fragment {
            fragment_index: i,
            total_n_fragments: size,
            data: slice_to_array(data, last as usize),
            length: last as u8,
        });
    }
    vec
}

fn compress(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

fn decompress(data: &[u8]) -> Vec<u8> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(Cursor::new(data));
    let mut output = Vec::new();
    decoder.read_to_end(&mut output).unwrap();
    output
}

#[cfg(test)]
mod test {

    use std::{
        fs::{self, File},
        io::{BufRead, BufReader},
    };

    use super::*;

    // Test string fragmentation
    #[test]
    fn test1() {
        let string = "hello".to_string();
        let ser = <String as Fragmentation<String>>::fragment(string);
        let ast = [1, 104, 101, 108, 108, 111].to_vec();
        eprintln!("{:?}\n{:?}", ast, ser);
        assert_eq!(ast, ser);
    }

    // Test serialization of string fragments
    #[test]
    fn test2() {
        let string = "hello".to_string();
        let fra = <String as Fragmentation<String>>::fragment(string);

        let mut ast = [0; 128];
        ast[0] = 104;
        ast[1] = 101;
        ast[2] = 108;
        ast[3] = 108;
        ast[4] = 111;

        let fr = Fragment {
            fragment_index: 2,
            total_n_fragments: 2,
            length: 5,
            data: ast,
        };
        let ser = serialize(fra);
        eprintln!("{:?}\n{:?}", fr, ser);

        for f in ser {
            if f.fragment_index != 1 {
                assert_eq!(f.data, fr.data);
                assert_eq!(f.fragment_index, fr.fragment_index);
                assert_eq!(f.length, fr.length);
                assert_eq!(f.total_n_fragments, fr.total_n_fragments);
            }
        }
    }

    // Test assembly of string fragments
    #[test]
    fn test3() {
        let dd = <String as Fragmentation<String>>::fragment("Hello".to_string());
        let mut dis = serialize(dd);
        let ass = <String as Assembler<String>>::assemble(&mut dis);
        if let Ok(rs) = ass {
            assert_eq!("Hello".to_string(), rs)
        } else {
            eprintln!("{:?}", ass.err());
            assert_eq!(1, 2)
        }
    }

    // Test sorting of fragments by index
    #[test]
    fn test4() {
        let fr0 = Fragment {
            fragment_index: 1,
            total_n_fragments: 4,
            length: 128,
            data: [0; 128],
        };
        let fr1 = Fragment::from_string(2, 4, "Hello".to_string());
        let fr2 = Fragment::from_string(3, 4, " World!\n".to_string());
        let fr3 = Fragment::from_string(4, 4, "Great!".to_string());
        let mut test_sub = vec![fr2, fr3, fr1, fr0];

        sort_by_fragment_index(&mut test_sub);
        for i in 1..test_sub.len() + 1 {
            assert_eq!(i, test_sub[i - 1].fragment_index as usize);
        }
    }

    #[test]
    fn test5() {
        let img = image::open("./assets/test/media/image/drone.png").expect("Failed to open image");

        let frags =
            <image::DynamicImage as Fragmentation<image::DynamicImage>>::fragment(img.clone());
        let mut series = serialize(frags.clone());
        let assembly: Result<DynamicImage, String> =
            <DynamicImage as Assembler<DynamicImage>>::assemble(&mut series);
        if let Ok(ass) = assembly.clone() {
            println!(
                "N_frag :{}\nDimension of reconstructed image{:?}\n Dim original h:{}__w:{}",
                series[0].total_n_fragments,
                ass.dimensions(),
                img.height(),
                img.width()
            );
        } else {
            println!("{:?}", assembly.clone().err());
        }
        assert_eq!(img, assembly.clone().ok().unwrap());
    }

    #[test]
    fn test6() {
        let def_req = DefaultsRequest::REGISTER;
        let def_bytes = <DefaultsRequest as Fragmentation<DefaultsRequest>>::fragment(def_req);
        let mut def_frag = serialize(def_bytes);
        if def_frag[0].fragment_index == 1 && def_frag[0].data[0] == 4 {
            let assembly = <DefaultsRequest as Assembler<DefaultsRequest>>::assemble(&mut def_frag);
            if let Ok(res) = assembly.clone() {
                println!("{:?}", res);
            } else {
                eprintln!("Something went wrong {:?}", assembly.clone().err());
            }
            assert_eq!(assembly.clone().ok().unwrap(), def_req);
        } else {
            eprintln!("Fragmentation went very wrong");
        }
    }

    #[test]
    fn test7() {
        let cr = ContentRequest::GETMEDIA("/home/sick7".to_string());
        let bytes = <ContentRequest as Fragmentation<ContentRequest>>::fragment(cr.clone());
        let fr = &mut serialize(bytes.clone());
        let asmbly = <ContentRequest as Assembler<ContentRequest>>::assemble(&mut fr.clone());
        if asmbly.clone().is_ok() {
            assert_eq!(cr.clone(), asmbly.clone().ok().unwrap());
        } else {
            println!("{:?}", asmbly.err().unwrap());
            assert_eq!(1, 2);
        }
    }

    #[test]
    fn test8() {
        let cr = ChatMessages::CHATSTRING(11, 21, 12, "Hello".to_string());
        let bytes = <ChatMessages as Fragmentation<ChatMessages>>::fragment(cr);
        let fr = serialize(bytes.clone());
        let asmb = <ChatMessages as Assembler<ChatMessages>>::assemble(&mut fr.clone());
        if asmb.is_ok() {
            match asmb.ok().unwrap() {
                ChatMessages::CHATSTRING(src, srv, dst, msg) => {
                    assert_eq!(src, 11);
                    assert_eq!(srv, 21);
                    assert_eq!(dst, 12);
                    assert_eq!("Hello".to_string(), msg);
                }
                _ => {}
            }
        } else {
            println!("{:?}", asmb.err());
            assert_eq!(1, 2);
        }
    }

    #[test]
    fn test9() {
        let dfrsp = DefaultResponse::REGISTERED(true, 1);
        let fr = <DefaultResponse as Fragmentation<DefaultResponse>>::fragment(dfrsp.clone());
        let ser = serialize(fr.clone());
        let asmb = <DefaultResponse as Assembler<DefaultResponse>>::assemble(&mut ser.clone());
        if asmb.is_ok() {
            match asmb.ok().unwrap() {
                DefaultResponse::REGISTERED(val, id) => match dfrsp.clone() {
                    DefaultResponse::REGISTERED(v, id1) => {
                        assert_eq!((val, id), (v, id1));
                        println!("Something went very wrong");
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    #[test]
    fn test10() {
        let dfrsp = DefaultResponse::ALLTEXT(
            [
                "Hello".to_string(),
                "world".to_string(),
                "!".to_string(),
                "Or".to_string(),
                "Something like that".to_string(),
            ]
            .to_vec(),
        );
        let fr = <DefaultResponse as Fragmentation<DefaultResponse>>::fragment(dfrsp.clone());
        let ser = serialize(fr);
        let asmb = <DefaultResponse as Assembler<DefaultResponse>>::assemble(&mut ser.clone());
        if asmb.clone().is_ok() {
            match asmb.clone().ok().unwrap() {
                DefaultResponse::ALLTEXT(val) => match dfrsp.clone() {
                    DefaultResponse::ALLTEXT(v) => {
                        assert_eq!(val.clone(), v.clone());
                        eprintln!("{:?}\n{:?}", val.clone(), v.clone());
                    }
                    _ => {}
                },
                _ => {}
            }
        } else {
            eprintln!("{}", asmb.err().unwrap());
            assert_eq!(1, 2);
        }
    }

    fn read_file_to_lines(file_path: &str) -> Result<Vec<String>, std::io::Error> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        // Handle potential errors when collecting lines
        reader
            .lines()
            .collect::<Result<Vec<String>, std::io::Error>>() // Use `Result::collect` to handle errors
    }

    #[test]
    fn test11() {
        let file = read_file_to_lines("./assets/test/text/test.txt").expect("Not Found");

        let dfrsp: DefaultResponse = DefaultResponse::ALLMEDIALINKS(file.clone());
        let fr = <DefaultResponse as Fragmentation<DefaultResponse>>::fragment(dfrsp.clone());
        let ser = serialize(fr);
        let asmb = <DefaultResponse as Assembler<DefaultResponse>>::assemble(&mut ser.clone());
        if asmb.is_ok() {
            match asmb.ok().unwrap() {
                DefaultResponse::ALLMEDIALINKS(val) => match dfrsp.clone() {
                    DefaultResponse::ALLMEDIALINKS(v) => {
                        eprintln!("{:?}\n{:?}", val.clone(), v.clone());
                        assert_eq!(val, v);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    #[test]
    fn test12() {
        let dfrsp = DefaultResponse::ALLAVAILABLE([11, 22, 44, 55].to_vec());
        let fr = <DefaultResponse as Fragmentation<DefaultResponse>>::fragment(dfrsp.clone());
        let ser = serialize(fr);
        let asmb = <DefaultResponse as Assembler<DefaultResponse>>::assemble(&mut ser.clone());
        match asmb {
            Ok(df) => match df.clone() {
                DefaultResponse::ALLAVAILABLE(ids) => {
                    eprintln!("{:?}", ids);
                    assert_eq!(ids, [11, 22, 44, 55].to_vec());
                }
                _ => {}
            },
            Err(e) => {
                eprintln!("{}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test13() {
        let dfrsp = DefaultResponse::SERVERTYPE(1, 12);
        let fr = <DefaultResponse as Fragmentation<DefaultResponse>>::fragment(dfrsp.clone());
        let ser = serialize(fr);
        let asmb = <DefaultResponse as Assembler<DefaultResponse>>::assemble(&mut ser.clone());
        if asmb.is_ok() {
            match asmb.ok().unwrap() {
                DefaultResponse::SERVERTYPE(val, id) => match dfrsp.clone() {
                    DefaultResponse::SERVERTYPE(v, id1) => {
                        assert_eq!((val, id), (v, id1));
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    #[test]
    fn test14() {
        let track_bytes =
            fs::read("./assets/test/media/audio/track_1.mp3").expect("File not found");
        let track = AudioSource {
            bytes: Arc::from(track_bytes),
        };
        let fr = <AudioSource as Fragmentation<AudioSource>>::fragment(track.clone());

        let ser = serialize(fr.clone());

        let asmb = <AudioSource as Assembler<AudioSource>>::assemble(&mut ser.clone());

        match asmb {
            Ok(tr) => {
                eprintln!("Track reconstruction gone wrong");
                eprintln!("{}", ser[0].total_n_fragments);
                assert_eq!(track.clone().bytes, tr.clone().bytes);
            }
            Err(s) => {
                eprintln!("{}", s);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test15() {
        let dfrsp = ContentResponse::TEXT(
            [
                "Hello".to_string(),
                "world".to_string(),
                "!".to_string(),
                "Or".to_string(),
                "Something like that".to_string(),
            ]
            .to_vec(),
        );
        let fr = <ContentResponse as Fragmentation<ContentResponse>>::fragment(dfrsp.clone());
        let ser = serialize(fr);
        let asmb = <ContentResponse as Assembler<ContentResponse>>::assemble(&mut ser.clone());
        if asmb.is_ok() {
            match asmb.ok().unwrap() {
                ContentResponse::TEXT(val) => match dfrsp.clone() {
                    ContentResponse::TEXT(v) => {
                        eprintln!("{:?}\n{:?}", val.clone(), v.clone());
                        assert_eq!(val, v);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    #[test]
    fn test16() {
        let dfrsp = ContentResponse::MEDIAIMAGE(
            image::open("./assets/test/media/image/drone.png")
                .ok()
                .unwrap(),
        );
        let fr = <ContentResponse as Fragmentation<ContentResponse>>::fragment(dfrsp.clone());
        let ser = serialize(fr);
        let asmb = <ContentResponse as Assembler<ContentResponse>>::assemble(&mut ser.clone());
        if asmb.is_ok() {
            match asmb.ok().unwrap() {
                ContentResponse::MEDIAIMAGE(val) => match dfrsp.clone() {
                    ContentResponse::MEDIAIMAGE(v) => {
                        eprintln!("{:?}\n{:?}", val.clone(), v.clone());
                        assert_eq!(val, v);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}
