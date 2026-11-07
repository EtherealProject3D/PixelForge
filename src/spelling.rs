//! Background Windows spelling checks; prompt text is never auto-corrected.
use std::{collections::HashSet, ops::Range, sync::mpsc, time::{Duration, Instant}};
use egui::{Color32, Stroke};

#[derive(Clone, Debug)]
struct Issue { range: Range<usize>, word: String, suggestions: Vec<String> }
struct Request { field: usize, revision: u64, text: String }
struct Reply { field: usize, revision: u64, text: String, result: Result<Vec<Issue>,String> }
struct Service { tx: mpsc::Sender<Request>, rx: mpsc::Receiver<Reply> }
impl Service {
    fn new(ctx: egui::Context) -> Self {
        let (tx,requests)=mpsc::channel::<Request>();
        let (replies,rx)=mpsc::channel();
        std::thread::spawn(move || {
            let checker=native::Checker::new();
            while let Ok(request)=requests.recv() {
                let result=match &checker {Ok(checker)=>checker.check(&request.text),Err(error)=>Err(error.clone())};
                if replies.send(Reply {field:request.field,revision:request.revision,text:request.text,result}).is_err() {break;}
                ctx.request_repaint();
            }
        });
        Self {tx,rx}
    }
}
struct Field {
    observed: String, checked: String, revision: u64, changed: Instant,
    pending: bool, issues: Vec<Issue>, error: Option<String>, menu_issue: Option<Issue>,
}
impl Default for Field {
    fn default() -> Self {Self {observed:String::new(),checked:String::new(),revision:0,changed:Instant::now(),pending:false,issues:Vec::new(),error:None,menu_issue:None}}
}
pub(crate) const SCRATCHPAD_FIELD: usize = 4;

pub struct PromptSpelling {
    pub enabled: bool,
    fields: [Field; SCRATCHPAD_FIELD + 1], service: Option<Service>, ignored: HashSet<String>,
}
impl Default for PromptSpelling {
    fn default() -> Self {Self {enabled:true,fields:Default::default(),service:None,
        ignored:["comfyui","sdxl","qwen","img2img","safetensors","lora","cfg","realesrgan"].into_iter().map(str::to_owned).collect()}}
}
impl PromptSpelling {
    pub(crate) fn editor_id(ui: &egui::Ui, field_id: usize) -> egui::Id {
        if field_id == SCRATCHPAD_FIELD {
            ui.make_persistent_id("scratchpad-text")
        } else {
            ui.make_persistent_id(egui::Id::new(("prompt_spelling", field_id)))
        }
    }
    pub fn edit(&mut self, ui: &mut egui::Ui, text: &mut String, field_id: usize, hint: &str, rows: usize) {
        if self.enabled && self.service.is_none() {self.service=Some(Service::new(ui.ctx().clone()));}
        if let Some(service)=&self.service {
            while let Ok(reply)=service.rx.try_recv() {
                let field=&mut self.fields[reply.field];
                if reply.revision==field.revision && reply.text==field.observed {
                    field.pending=false;field.checked=reply.text;
                    match reply.result {Ok(issues)=>{field.issues=issues;field.error=None;},Err(error)=>{field.issues.clear();field.error=Some(error);}}
                }
            }
        }
        let field=&mut self.fields[field_id];
        let issues:Vec<Issue>=if self.enabled && field.checked==*text {
            field.issues.iter().filter(|issue|!self.ignored.contains(&issue.word.to_lowercase())).cloned().collect()
        } else {Vec::new()};
        let checked=field.checked.clone();
        let mut layouter=|ui:&egui::Ui,buffer:&dyn egui::TextBuffer,width:f32| {
            let value=buffer.as_str();
            let normal=egui::TextFormat {font_id:egui::TextStyle::Body.resolve(ui.style()),color:ui.visuals().text_color(),..Default::default()};
            let mut job = egui::text::LayoutJob::simple(
                String::new(), normal.font_id.clone(), normal.color, width,
            );
            job.sections.clear();
            let mut offset=0;
            if value==checked {
                for issue in &issues {
                    if issue.range.start<offset || value.get(issue.range.clone()).is_none() {continue;}
                    job.append(&value[offset..issue.range.start],0.0,normal.clone());
                    let mut marked=normal.clone();marked.underline=Stroke::new(1.3_f32,Color32::LIGHT_RED);
                    job.append(&value[issue.range.clone()],0.0,marked);
                    offset=issue.range.end;
                }
            }
            job.append(&value[offset..],0.0,normal);
            ui.fonts(|fonts|fonts.layout_job(job))
        };
        let editor = egui::TextEdit::multiline(text)
            .id(Self::editor_id(ui, field_id))
            .hint_text(hint)
            .desired_width(f32::INFINITY)
            .desired_rows(rows);
        let output = if issues.is_empty() {
            editor.show(ui)
        } else {
            editor.layouter(&mut layouter).show(ui)
        };
        if field_id == SCRATCHPAD_FIELD {
            if output.response.clicked() {
                output.response.request_focus();
            }
            // egui skips its caret when the viewport reports no native focus.
            // Keep the insertion point visible while the scratchpad owns keyboard focus.
            if ui.memory(|memory| memory.has_focus(output.response.id))
                && !ui.input(|input| input.focused)
                && let Some(selection) = output.cursor_range
            {
                let caret = output
                    .galley
                    .pos_from_cursor(selection.primary)
                    .translate(output.galley_pos.to_vec2());
                let top = caret.center_top();
                let height = ui.fonts(|fonts| fonts.row_height(&egui::TextStyle::Body.resolve(ui.style())));
                let bottom = top + egui::vec2(0.0, caret.height().max(height));
                ui.painter_at(output.text_clip_rect)
                    .line_segment([top, bottom], ui.visuals().text_cursor.stroke);
            }
        }
        if output.response.secondary_clicked() {
            if field_id == SCRATCHPAD_FIELD {
                output.response.request_focus();
            }
            field.menu_issue=output.response.interact_pointer_pos().and_then(|pointer| {
                let cursor=output.galley.cursor_from_pos(pointer-output.galley_pos);
                let byte=text.char_indices().nth(cursor.index).map(|(byte,_)|byte).unwrap_or(text.len());
                issues.iter().find(|issue|issue.range.contains(&byte) || issue.range.end==byte).cloned()
            });
        }
        let mut correction:Option<(Issue,String)>=None;
        let mut ignore=None;
        let issue=field.menu_issue.clone().filter(|_|field.checked==*text && self.enabled);
        crate::interaction::text_menu(ui,&output.response,text,&output.state,|ui| {
            if let Some(issue)=&issue {ui.separator();suggestions(ui,issue,&mut correction,&mut ignore);}
        });
        if let Some((issue, replacement)) = correction {
            apply_correction(text, &field.checked, &issue, &replacement);
            output.response.request_focus();
        }
        if let Some(word) = ignore {
            self.ignored.insert(word.to_lowercase());
            output.response.request_focus();
        }
        if field.observed!=*text {
            field.observed=text.clone();field.changed=Instant::now();field.revision=field.revision.wrapping_add(1);
            field.pending=false;field.menu_issue=None;field.error=None;
        }
        if self.enabled && !text.is_empty() && field.checked!=*text && !field.pending {
            let delay=Duration::from_millis(400);
            if field.changed.elapsed()>=delay {
                if let Some(service)=&self.service {
                    field.pending=service.tx.send(Request {field:field_id,revision:field.revision,text:text.clone()}).is_ok();
                    if !field.pending {field.error=Some("Spelling service stopped. Restart PixelForge to try again.".into());field.checked=text.clone();}
                }
            } else {ui.ctx().request_repaint_after(delay.saturating_sub(field.changed.elapsed()));}
        }
        if field.pending {ui.ctx().request_repaint_after(Duration::from_millis(100));}
        if self.enabled {if let Some(error)=&field.error {ui.small(error);}}
    }
}
fn suggestions(ui:&mut egui::Ui,issue:&Issue,correction:&mut Option<(Issue,String)>,ignore:&mut Option<String>) {
    ui.strong(format!("Spelling: {}",issue.word));
    if issue.suggestions.is_empty() {ui.label("No suggestions available.");}
    for replacement in &issue.suggestions {
        let label=if replacement.is_empty(){"Remove repeated word"}else{replacement};
        if ui.button(label).clicked() {*correction=Some((issue.clone(),replacement.clone()));ui.close();}
    }
    ui.separator();
    if ui.button("Ignore for this session").clicked() {*ignore=Some(issue.word.clone());ui.close();}
}
fn apply_correction(text:&mut String,checked:&str,issue:&Issue,replacement:&str) -> bool {
    if text!=checked || text.get(issue.range.clone())!=Some(issue.word.as_str()) {return false;}
    text.replace_range(issue.range.clone(),replacement);true
}
fn utf16_range(text:&str,start:u32,length:u32)->Option<Range<usize>> {
    fn byte_offset(text:&str,target:usize)->Option<usize> {
        let mut units=0;
        for (byte,ch) in text.char_indices() {if units==target{return Some(byte);}units+=ch.len_utf16();}
        (units==target).then_some(text.len())
    }
    let end=start.checked_add(length)?;
    Some(byte_offset(text,start as usize)?..byte_offset(text,end as usize)?)
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows::{core::{w,HSTRING,PWSTR},Win32::{Globalization::{ISpellChecker,ISpellCheckerFactory,SpellCheckerFactory,CORRECTIVE_ACTION_DELETE,CORRECTIVE_ACTION_REPLACE,CORRECTIVE_ACTION_NONE},System::Com::{CoCreateInstance,CoInitializeEx,CoTaskMemFree,CoUninitialize,CLSCTX_INPROC_SERVER,COINIT_APARTMENTTHREADED}}};
    // The checker and COM apartment are created, used and destroyed only on this worker.
    pub struct Checker {checker:Option<ISpellChecker>}
    impl Checker {
        pub fn new()->Result<Self,String> {
            unsafe {
                CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().map_err(|e|e.to_string())?;
                let result=(||->windows::core::Result<ISpellChecker>{
                    let factory:ISpellCheckerFactory=CoCreateInstance(&SpellCheckerFactory,None,CLSCTX_INPROC_SERVER)?;
                    factory.CreateSpellChecker(w!("en-US"))
                })();
                match result {Ok(checker)=>Ok(Self{checker:Some(checker)}),Err(error)=>{CoUninitialize();Err(format!("English spell check is unavailable. Install English (United States) language support in Windows. ({error})"))}}
            }
        }
        pub fn check(&self,text:&str)->Result<Vec<Issue>,String> {
            if text.chars().count()>32_000 {return Err("Spell check supports prompts up to 32,000 characters.".into());}
            if text.contains('\0') {return Err("Spell check cannot process embedded null characters.".into());}
            unsafe {self.check_inner(text).map_err(|e|format!("Spell check failed: {e}"))}
        }
        unsafe fn check_inner(&self,text:&str)->windows::core::Result<Vec<Issue>> {
            let checker=self.checker.as_ref().expect("active COM checker");
            let errors=unsafe{checker.Check(&HSTRING::from(text))?};
            let mut issues=Vec::new();
            for _ in 0..64 {
                let mut error=None;
                unsafe{errors.Next(&mut error).ok()?;}
                let Some(error)=error else {break};
                let action=unsafe{error.CorrectiveAction()?};
                if action==CORRECTIVE_ACTION_NONE {continue;}
                let Some(range)=utf16_range(text,unsafe{error.StartIndex()?},unsafe{error.Length()?}) else {continue};
                let Some(word)=text.get(range.clone()).filter(|word|!word.is_empty()) else {continue};
                let mut suggestions=Vec::new();
                if action==CORRECTIVE_ACTION_DELETE {suggestions.push(String::new());}
                else if action==CORRECTIVE_ACTION_REPLACE {
                    let pointer=unsafe{error.Replacement()?};
                    if !pointer.is_null() {suggestions.push(unsafe{take_string(pointer)}?);}
                } else if let Ok(values)=unsafe{checker.Suggest(&HSTRING::from(word))} {
                    for _ in 0..6 {
                        let mut slot=[PWSTR::null()];let mut fetched=0;
                        let status=unsafe{values.Next(&mut slot,Some(&mut fetched))};
                        if fetched>0 && !slot[0].is_null() {suggestions.push(unsafe{take_string(slot[0])}?);}
                        status.ok()?;
                        if fetched==0 {break;}
                    }
                }
                issues.push(Issue {range,word:word.to_owned(),suggestions});
            }
            issues.sort_by_key(|issue|issue.range.start);
            Ok(issues)
        }
    }
    unsafe fn take_string(pointer:PWSTR)->windows::core::Result<String> {
        let result=unsafe{pointer.to_string()};
        unsafe{CoTaskMemFree(Some(pointer.0.cast()));}
        result.map_err(Into::into)
    }
    impl Drop for Checker {fn drop(&mut self){self.checker.take();unsafe{CoUninitialize();}}}
}
#[cfg(not(windows))]
mod native {
    use super::*;
    pub struct Checker;
    impl Checker {
        pub fn new()->Result<Self,String>{Err("Prompt spell checking currently requires Windows.".into())}
        pub fn check(&self,_:&str)->Result<Vec<Issue>,String>{Ok(Vec::new())}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_scratchpad_paints_caret_with_and_without_spelling_marks() {
        for (input, marked, focused) in [
            ("", false, true),
            ("mispeling", true, true),
            ("red coat", false, false),
        ] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let mut spelling = PromptSpelling::default();
            let mut draft = input.to_owned();
            if marked {
                let field = &mut spelling.fields[SCRATCHPAD_FIELD];
                field.checked = draft.clone();
                field.observed = draft.clone();
                field.issues.push(Issue {
                    range: 0..draft.len(),
                    word: draft.clone(),
                    suggestions: vec!["misspelling".to_owned()],
                });
            }
            let output = ctx.run(egui::RawInput { focused, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let id = PromptSpelling::editor_id(ui, SCRATCHPAD_FIELD);
                    ui.memory_mut(|memory| memory.request_focus(id));
                    spelling.edit(ui, &mut draft, SCRATCHPAD_FIELD, "Draft a prompt here…", 6);
                });
            });
            assert!(output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::LineSegment { points, stroke }
                    if points[0].x == points[1].x && stroke.color == Color32::WHITE && stroke.width == 2.0
            )), "missing caret for {input:?}");
        }
    }
    #[test]
    fn utf16_offsets_handle_emoji_and_accents() {
        let text="🦊 café teh";
        assert_eq!(&text[utf16_range(text,8,3).unwrap()],"teh");
        assert!(utf16_range(text,1,1).is_none());
        assert!(utf16_range(text,500,2).is_none());
    }
    #[test]
    fn correction_changes_only_the_chosen_occurrence_and_rejects_stale_text() {
        let original="teh fox and teh moon";
        let issue=Issue {range:12..15,word:"teh".into(),suggestions:vec!["the".into()]};
        let mut text=original.to_owned();
        assert!(apply_correction(&mut text,original,&issue,"the"));
        assert_eq!(text,"teh fox and the moon");
        assert!(!apply_correction(&mut text,original,&issue,"the"));
    }
    #[cfg(windows)]
    #[test]
    fn installed_windows_checker_finds_a_typo_and_provides_suggestions() {
        let checker=native::Checker::new().expect("Windows English spell checker");
        let issues=checker.check("An image with speling mistakes.").unwrap();
        let issue=issues.iter().find(|issue|issue.word=="speling").expect("misspelling should be detected");
        assert!(!issue.suggestions.is_empty());
    }
}
