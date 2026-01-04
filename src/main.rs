// src/main.rs

use std::io::{BufRead, Write};

//
// constants
//
const MAX_WEIGHT_UDM: i16 = 250;
const DEFAULT_WEIGHT_UDM: i16 = MAX_WEIGHT_UDM;
const MAX_WEIGHT_ASK: i16 = 128;
const OUTPUT_FILE_UDM: &str = "merged_output_udm.txt";

//
// main program structs
//
#[derive(Clone, Debug)]
struct Record {
    word: String,
    weight_udm: i16,
    _language_code: String, // currently unused
    supplemental_data: Vec<String>,
}

//
// file type traits
//
trait FileTypeRead {
    fn read_file(path: &str) -> Result<Vec<Record>, String>;
}

trait FileTypeWrite {
    fn write_file(path: &str, records: Vec<Record>) -> Result<(), String>;
}

//
// file type classes
//
struct AnySoftKeyboardFile;
impl FileTypeRead for AnySoftKeyboardFile {
    fn read_file(path: &str) -> Result<Vec<Record>, String> {
        // file is in an XML format
        let file_content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {e:?}"))?;
        let doc = roxmltree::Document::parse(&file_content)
            .map_err(|e| format!("Failed to parse XML: {e:?}"))?;

        let mut records: Vec<Record> = vec![];
        for el in doc
            .root()
            .children()
            // fetching 'AnySoftKeyboardPrefs' elements
            .filter(|el| el.is_element() && el.has_tag_name("AnySoftKeyboardPrefs"))
            .collect::<Vec<_>>()
            .first()
            .ok_or("Missing 'AnySoftKeyboardPrefs' element".to_string())?
            .children()
            // fetching 'AnySoftKeyboardPrefs.pref' elements
            .filter(|el| el.is_element() && el.has_tag_name("pref"))
            .collect::<Vec<_>>()
            .first()
            .ok_or("Missing 'AnySoftKeyboardPrefs.pref' element".to_string())?
            .children()
            // fetching 'AnySoftKeyboardPrefs.pref.pref' elements
            .filter(|el| el.is_element() && el.has_tag_name("pref"))
            .collect::<Vec<_>>()
            .first()
            .ok_or("Missing 'AnySoftKeyboardPrefs.pref.pref' element".to_string())?
            .children()
            // fetching 'AnySoftKeyboardPrefs.pref.pref.pref' elements
            .filter(|el| el.is_element() && el.has_tag_name("pref"))
        {
            // fresh record with default or empty values but language code from 'AnySoftKeyboardPrefs.pref.pref.value' element already set
            let rec: Record = Record {
                word: String::from(""),
                weight_udm: DEFAULT_WEIGHT_UDM,
                _language_code: match el
                    .children()
                    .filter(|child| child.is_element() && child.has_tag_name("value"))
                    .collect::<Vec<_>>()
                    .first()
                    .ok_or("Missing 'AnySoftKeyboardPrefs.pref.pref.value' element".to_string())?
                    .attribute("locale")
                {
                    Some(lang_code) => lang_code.to_string(),
                    None => String::from(""), // leave as default empty string if no language code is present
                },
                supplemental_data: vec![],
            };

            for el in el
                .children()
                .filter(|child| child.is_element() && child.has_tag_name("pref"))
            {
                let mut rec_write = rec.clone();
                // fetching first 'AnySoftKeyboardPrefs.pref.pref.value' element with attribute 'locale' for language code
                for (att_name, att_value_str) in el
                    .children()
                    .filter(|child| child.is_element() && child.has_tag_name("value"))
                    .map(|child| {
                        return match child.attributes().collect::<Vec<_>>().first() {
                            Some(a) => (a.name(), a.value()),
                            None => ("", ""),
                        };
                    })
                {
                    match att_name {
                        "word" => rec_write.word = att_value_str.to_string(),
                        "freq" => {
                            rec_write.weight_udm = match att_value_str.parse::<f32>() {
                                Ok(freq) => (((freq / (MAX_WEIGHT_ASK as f32))
                                    * (MAX_WEIGHT_UDM as f32))
                                    .round() as i16)
                                    .clamp(0, MAX_WEIGHT_UDM),
                                Err(_) => DEFAULT_WEIGHT_UDM,
                            }
                        }
                        &_ => {
                            // ignore other attributes for now
                        }
                    }
                }

                if !rec_write.word.is_empty() {
                    records.push(rec_write);
                }
            }
        }

        print!(
            "Read {} records from AnySoftKeyboard file.\n",
            records.len()
        );

        // Placeholder implementation
        Ok(records)
    }
}

struct UDMFile;
impl FileTypeRead for UDMFile {
    fn read_file(path: &str) -> Result<Vec<Record>, String> {
        // get lines from file
        let file = std::fs::File::open(path).map_err(|e| format!("Failed to open file: {e:?}"))?;
        let reader = std::io::BufReader::new(file);
        let lines_collect_res: Result<Vec<String>, _> = reader.lines().collect();
        let lines = lines_collect_res.map_err(|e| format!("Failed to read lines: {e:?}"))?;

        let records_iterated = lines.iter().map({
            |line: &String| {
                let fields: Vec<&str> = line.split('|').collect();

                // extract weight if present
                let last_field = fields.last().unwrap_or(&"");
                let weight = if last_field.contains("weight:") {
                    last_field
                        .split(":")
                        .nth(1)
                        .unwrap_or(DEFAULT_WEIGHT_UDM.to_string().as_str())
                        .parse::<i16>()
                        .unwrap_or(DEFAULT_WEIGHT_UDM)
                } else {
                    DEFAULT_WEIGHT_UDM
                };

                return Record {
                    word: fields[0].to_string(), // first field is the word
                    weight_udm: weight,
                    _language_code: String::from(""), // no language codes are contained in UDM files (to my knowledge)
                    supplemental_data: fields[1..fields.len() - 1]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(), // any fields between word and weight (first and last, respectively)
                };
            }
        });

        print!("Read {} records from UDM file.\n", lines.len());

        Ok(records_iterated.collect())
    }
}
impl FileTypeWrite for UDMFile {
    fn write_file(path: &str, records: Vec<Record>) -> Result<(), String> {
        let mut file = std::fs::File::create(path)
            .map_err(|e| format!("Failed to create output file: {e:?}"))?;
        for record in &records {
            let line = format!(
                "{}|{}|weight:{}\n",
                record.word,
                record.supplemental_data.join(","),
                record.weight_udm
            );
            file.write_all(line.as_bytes())
                .map_err(|e| format!("Failed to write line: {e:?}"))?;
        }

        print!("Wrote {} records to UDM output file.\n", records.len());

        Ok(())
    }
}

//
// RecordProcessor class
//
struct RecordProcessor {
    records: Vec<Record>,
}
impl RecordProcessor {
    fn new(records: Vec<Record>) -> Self {
        Self { records }
    }

    fn append_records(&mut self, new_records: Vec<Record>) {
        self.records.extend(new_records);
    }

    fn sort_alphabetically(&mut self) {
        self.records.sort_by(|a, b| a.word.cmp(&b.word));
    }

    fn sort_alphabetically_case_insensitive(&mut self) {
        self.records
            .sort_by(|a, b| a.word.to_lowercase().cmp(&b.word.to_lowercase()));
    }

    // needs to be sorted first
    fn remove_duplicates_case_insensitive(&mut self) {
        let len_before = self.records.len();
        self.records.dedup_by_key(|r| r.word.clone().to_lowercase());
        let len_after = self.records.len();
        println!("Removed {} duplicate records.", len_before - len_after);
    }
}

//
// main function
//
fn main() -> Result<(), String> {
    // print directory the program is running in
    println!(
        "Current directory: {:?}",
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {e:?}"))? // effectively converting from "Error" to "String" format
    );

    // gather all records from different files
    let mut processor: RecordProcessor = RecordProcessor::new(vec![]);
    processor.append_records(AnySoftKeyboardFile::read_file(
        "AnySoftKeyboardPrefs_UserWords.xml",
    )?);
    processor.append_records(UDMFile::read_file("udm_words_export.txt")?);

    // preprocess records
    processor.sort_alphabetically_case_insensitive();
    processor.remove_duplicates_case_insensitive();
    processor.sort_alphabetically();

    // write back merged output
    UDMFile::write_file(OUTPUT_FILE_UDM, processor.records)?;

    Ok(())
}
