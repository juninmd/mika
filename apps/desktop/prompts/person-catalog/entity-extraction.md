You extract PEOPLE MEMORY from one message the user sent to {{characterName}}. Output only JSON, no prose.

Record people (and named characters/organizations) the user talks about, how they relate to the user, and durable facts about them. The user may write in Portuguese or English — handle both.

Operations (each item of "ops"):
- {"op":"upsert_person","ref":REF,"type":TYPE} — someone is mentioned.
- {"op":"add_fact","ref":REF,"fact":TEXT,"confidence":0-1} — a durable fact about that person (likes, dislikes, job, where they live, habits, birthday, pets…).
- {"op":"set_name","ref":REF,"name":NAME} — the user reveals the name of someone known only by relation ("minha esposa se chama Dani" → ref {"relation":"esposa"}, name "Dani").
- {"op":"add_alias","ref":REF,"alias":TEXT} — another name/nickname for the same person.
- {"op":"ask_clarification","ref":REF} — the user refers to someone by a relation that fits several known people and does not say which one.

Links BETWEEN two named people (never involving the user) go in a top-level "relationships" list: {"from":NAME,"to":NAME,"label":TEXT} ("o João trabalha com a Maria" → {"from":"João","to":"Maria","label":"trabalha com"}).

REF is {"name":NAME?,"relation":RELATION?}: the person's name if said, and their relation TO THE USER if said ("esposa", "irmão", "chefe", "wife", "brother"). A relation-only REF is fine: "minha esposa gosta de sorvete" → {"op":"add_fact","ref":{"relation":"esposa"},"fact":"gosta de sorvete","confidence":0.9}.
TYPE is one of: real_person, fictional_character, organization, pet, streamer, vtuber, singer, athlete, author, game, anime, movie, series, other.

Rules:
- Store names, relations and facts exactly as the user said them — same words, same language. Never translate, never "correct" a name.
- Never invent. No person, relation or fact that is not in the message. Hedged or temporary things ("talvez", "hoje ela está cansada") are not facts. Facts about the user themselves are not your job.
- Never include the user nor {{characterName}} as a person.
- Plural relations ("meus irmãos são o Pedro e o João") become one upsert_person per named person, each with the singular relation ("irmão").
- If the message answers an open question below ("o Pedro"), emit an upsert_person for the person it names, with that relation.
- Nothing about people → {"ops":[],"relationships":[]}.

Known people (use the same names; "unnamed" means only the relation is known):
{{knownPeople}}

Open questions (the user was asked which person they meant):
{{pending}}

{{appContext}}

Example — "Minha esposa se chama Dani e ela adora Stardew Valley. Meu irmão João é médico." →
{"ops":[{"op":"set_name","ref":{"relation":"esposa"},"name":"Dani"},{"op":"add_fact","ref":{"name":"Dani","relation":"esposa"},"fact":"adora Stardew Valley","confidence":0.9},{"op":"upsert_person","ref":{"name":"João","relation":"irmão"},"type":"real_person"},{"op":"add_fact","ref":{"name":"João","relation":"irmão"},"fact":"é médico","confidence":0.9}],"relationships":[]}

Message: "{{text}}"
