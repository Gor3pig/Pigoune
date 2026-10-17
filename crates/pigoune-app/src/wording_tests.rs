const OBSOLETE_WORDING: &[&str] = &[
    "A mask is hidden, so the image would show what it covers",
    "“{name}” is hidden: the image will show what it covers.",
    "{count} masks are hidden: the image will show what they cover.",
    "A Mask Is Hidden",
    "Continue _Anyway",
    "_Show the Masks",
    "Show the hidden masks of “{name}” before dragging it out of Pigoune",
    "Show the hidden masks of {count} images before dragging them out of Pigoune",
    "The annotated image could not be rendered",
    "The export is partial. {exported} files were exported to “{folder}”, but {count} could not be created:\n\n{list}",
    "The export contains every original image, which may still show what the annotations hide, and a flattened copy of each annotated image.",
    "Un masquage est masqué : l'image montrerait ce qu'il cache",
    "«\u{a0}{name}\u{a0}» est masqué : l'image montrera ce qu'il cache.",
    "{count} masquages sont masqués : l'image montrera ce qu'ils cachent.",
    "Un masquage est masqué",
    "Continuer _quand même",
    "_Afficher les masquages",
    "Réaffichez les masquages cachés de «\u{a0}{name}\u{a0}» avant de le glisser hors de Pigoune",
    "Réaffichez les masquages cachés de {count} images avant de les glisser hors de Pigoune",
    "Eine Maskierung ist ausgeblendet: Das Bild würde zeigen, was sie verdeckt",
    "„{name}“ ist ausgeblendet: Das Bild zeigt, was es verdeckt.",
    "{count} Maskierungen sind ausgeblendet: Das Bild zeigt, was sie verdecken.",
    "Eine Maskierung ist ausgeblendet",
    "Trotzdem _fortfahren",
    "Maskierungen _einblenden",
    "Blenden Sie die ausgeblendeten Maskierungen von „{name}“ ein, bevor Sie das Bild aus Pigoune ziehen",
    "Blenden Sie die ausgeblendeten Maskierungen von {count} Bildern ein, bevor Sie sie aus Pigoune ziehen",
    "Un enmascaramiento está oculto: la imagen mostraría lo que cubre",
    "«{name}» está oculto: la imagen mostrará lo que cubre.",
    "{count} enmascaramientos están ocultos: la imagen mostrará lo que cubren.",
    "Un enmascaramiento está oculto",
    "Continuar de t_odos modos",
    "_Mostrar los enmascaramientos",
    "Muestre los enmascaramientos ocultos de «{name}» antes de arrastrarla fuera de Pigoune",
    "Muestre los enmascaramientos ocultos de {count} imágenes antes de arrastrarlas fuera de Pigoune",
    "Una mascheratura è nascosta: l'immagine mostrerebbe ciò che copre",
    "«{name}» è nascosto: l'immagine mostrerà ciò che copre.",
    "{count} mascherature sono nascoste: l'immagine mostrerà ciò che coprono.",
    "Una mascheratura è nascosta",
    "Continua _comunque",
    "_Mostra le mascherature",
    "Mostra le mascherature nascoste di «{name}» prima di trascinarla fuori da Pigoune",
    "Mostra le mascherature nascoste di {count} immagini prima di trascinarle fuori da Pigoune",
    "Een maskering is verborgen: de afbeelding zou tonen wat ze bedekt",
    "“{name}” is verborgen: de afbeelding toont wat het bedekt.",
    "{count} maskeringen zijn verborgen: de afbeelding toont wat ze bedekken.",
    "Een maskering is verborgen",
    "Toch _doorgaan",
    "Maskeringen _tonen",
    "Toon de verborgen maskeringen van “{name}” voordat u de afbeelding uit Pigoune sleept",
    "Toon de verborgen maskeringen van {count} afbeeldingen voordat u ze uit Pigoune sleept",
    "Maskowanie jest ukryte: obraz pokazałby to, co ono zakrywa",
    "„{name}” jest ukryte: obraz pokaże to, co ono zakrywa.",
    "Ukrytych maskowań: {count}. Obraz pokaże to, co one zakrywają.",
    "Maskowanie jest ukryte",
    "Mimo to _kontynuuj",
    "_Pokaż maskowania",
    "Pokaż ukryte maskowania obrazu „{name}”, zanim przeciągniesz go poza Pigoune",
    "Pokaż ukryte maskowania {count} obrazów, zanim przeciągniesz je poza Pigoune",
    "Uma máscara está oculta: a imagem mostraria o que ela cobre",
    "“{name}” está oculto: a imagem mostrará o que ele cobre.",
    "{count} máscaras estão ocultas: a imagem mostrará o que elas cobrem.",
    "Uma máscara está oculta",
    "Continuar _mesmo assim",
    "_Mostrar as máscaras",
    "Mostre as máscaras ocultas de “{name}” antes de arrastá-la para fora do Pigoune",
    "Mostre as máscaras ocultas de {count} imagens antes de arrastá-las para fora do Pigoune",
    "L'image annotée n'a pas pu être produite",
    "L'export contient toutes les images d'origine, qui peuvent encore montrer ce que les annotations cachent, et une copie aplatie de chaque image annotée.",
    "L'export est partiel. {exported} fichiers ont été exportés dans «\u{a0}{folder}\u{a0}», mais {count} n'ont pas pu être créés :\n\n{list}",
    "Das kommentierte Bild konnte nicht erzeugt werden",
    "Der Export enthält jedes Originalbild, das möglicherweise noch zeigt, was die Anmerkungen verdecken, und eine zusammengeführte Kopie jedes kommentierten Bilds.",
    "Der Export ist unvollständig. {exported} Dateien wurden nach „{folder}“ exportiert, aber {count} konnten nicht erstellt werden:\n\n{list}",
    "La exportación contiene todas las imágenes originales, que pueden mostrar aún lo que ocultan las anotaciones, y una copia aplanada de cada imagen anotada.",
    "La exportación es parcial. Se exportaron {exported} archivos a «{folder}», pero no se pudieron crear {count}:\n\n{list}",
    "L'esportazione contiene tutte le immagini originali, che possono ancora mostrare ciò che le annotazioni nascondono, e una copia appiattita di ogni immagine annotata.",
    "L'esportazione è parziale. {exported} file sono stati esportati in «{folder}», ma {count} non è stato possibile crearli:\n\n{list}",
    "De export bevat elke originele afbeelding, die nog kan tonen wat de annotaties verbergen, en een afgevlakte kopie van elke geannoteerde afbeelding.",
    "De export is gedeeltelijk. {exported} bestanden zijn geëxporteerd naar “{folder}”, maar {count} konden niet worden gemaakt:\n\n{list}",
    "Eksport zawiera każdy oryginalny obraz, który może nadal pokazywać to, co ukrywają adnotacje, oraz spłaszczoną kopię każdego obrazu z adnotacjami.",
    "Eksport jest częściowy. Wyeksportowano {exported} plików do „{folder}”, ale nie udało się utworzyć {count}:\n\n{list}",
    "A exportação contém todas as imagens originais, que ainda podem mostrar o que as anotações escondem, e uma cópia achatada de cada imagem anotada.",
    "A exportação é parcial. {exported} arquivos foram exportados para “{folder}”, mas {count} não puderam ser criados:\n\n{list}",
    "masquage est masqué",
    "masquages sont masqués",
    "montrerait ce qu’il cache",
    "montrerait ce qu'il cache",
];

const CATALOGS: [(&str, &str); 8] = [
    ("pigoune.pot", include_str!("../../../po/pigoune.pot")),
    ("fr.po", include_str!("../../../po/fr.po")),
    ("de.po", include_str!("../../../po/de.po")),
    ("es.po", include_str!("../../../po/es.po")),
    ("it.po", include_str!("../../../po/it.po")),
    ("nl.po", include_str!("../../../po/nl.po")),
    ("pl.po", include_str!("../../../po/pl.po")),
    ("pt_BR.po", include_str!("../../../po/pt_BR.po")),
];

const SOURCES: [(&str, &str); 5] = [
    ("mask_alert.rs", include_str!("mask_alert.rs")),
    ("drag_export.rs", include_str!("drag_export.rs")),
    ("editor.rs", include_str!("editor.rs")),
    ("window.rs", include_str!("window.rs")),
    ("editor.blp", include_str!("../ui/editor.blp")),
];

const SHOWN_WORDING: &[&str] = &[
    "Some Information May Be Visible",
    "A blur or pixelation effect is hidden.",
    "{count} blur or pixelation effects are hidden.",
    "A blur or pixelation effect is currently hidden.",
    "{count} blur or pixelation effects are currently hidden.",
    "“{name}” contains hidden blur or pixelation effects.",
    "{count} images contain hidden blur or pixelation effects.",
    "To drag “{name}” out of Pigoune, first show its hidden blur or pixelation effects.",
    "To drag {count} images out of Pigoune, first show their hidden blur or pixelation effects.",
    "_Show Hidden Effects",
];

fn joined(catalog: &str) -> String {
    catalog.replace("\"\n\"", "")
}

fn translation_of(catalog: &str, message: &str) -> Option<String> {
    let text = joined(catalog);
    let wanted = format!("msgid \"{message}\"\nmsgstr \"");
    let start = text.find(&wanted)? + wanted.len();
    let end = text[start..].find("\"\n")?;
    Some(text[start..start + end].to_owned())
}

#[test]
fn no_catalog_keeps_an_obsolete_message_about_hidden_masks() {
    for (name, catalog) in CATALOGS {
        let text = joined(catalog);
        for phrase in OBSOLETE_WORDING {
            let escaped = phrase.replace('"', "\\\"");
            assert!(
                !text.contains(&format!("\"{escaped}\""))
                    && !text.contains(&escaped.replace("\\n", "\n")),
                "{name} still has: {phrase}"
            );
        }
    }
}

#[test]
fn the_code_and_the_interface_files_no_longer_ask_for_an_obsolete_message() {
    for (name, source) in SOURCES {
        for phrase in OBSOLETE_WORDING.iter().filter(|phrase| phrase.is_ascii()) {
            assert!(!source.contains(phrase), "{name} still asks for: {phrase}");
        }
    }
}

#[test]
fn every_language_translates_the_messages_about_hidden_effects() {
    for (name, catalog) in CATALOGS.iter().filter(|(name, _)| *name != "pigoune.pot") {
        for message in SHOWN_WORDING {
            let translated = translation_of(catalog, message)
                .unwrap_or_else(|| panic!("{name} has no entry for: {message}"));
            assert!(
                !translated.is_empty(),
                "{name} does not translate: {message}"
            );
            assert_ne!(&translated, message, "{name} keeps the English: {message}");
            for placeholder in ["{name}", "{count}"]
                .iter()
                .filter(|p| message.contains(**p))
            {
                assert!(
                    translated.contains(placeholder),
                    "{name} loses {placeholder}: {message}"
                );
            }
        }
    }
}
