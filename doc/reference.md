Reference object

A reference is an entity that represents a way an object - a landmark - is referenced in a user text.

It has : 

- local_id : the index of this reference in the reference list for this trace mirror.
- expression : the exact expression the object has been referenced with
- description : some other attributes about the object
- title_suggestion : a best guess for a direct reference to the object
- reference_type : is the reference using a proper name, a description, an indexical...
- context_tags : Words that help fix the context of the description
- reference_variants : Close expressions to the recorded expression, that could have been used the same way.
- is_user_specific : Is the referenced entity specific to this user or could it be shared (Not sure it belongs to reference)

Relations : 
- trace_id
- trace_mirror_id 
One or the other, maybe trace_mirror is better because it is related to an analysis run.

- landmark_id : the landmark identified as referenced by the expression.
- analysis_id ?