I want the user to declare some high level projects in a meta trace.
It will allow better understanding of the user content (worked on code -> He has a project of getting better at tech -> it is about that.)

HighLevelProject will be landmarks for now.

User defines projects.

They are processed by a LLM call that gives a list of projects with :
- title
- one liner -> subtitle
- description

Futur fields : 
- area : WORK | PERSONAL | SPORT


First mirror pipeline stage ask if those HTL are present in the trace. Array of projects.
Here we should ask for a array of the full text spans where it talks about this. This way, we know in the following steps that the spans are about a given project.

Reference extraction : 
- do we want to have a link between landmarks and projects ? For now we don't.
We will create some strong relations later if we see that a deliverable is very important to a given project etc.

Grammatical extraction :
- The transactions will be linked to the HLP they belong to if any.


We use a trace with type :
HIGH_LEVEL_PROJECTS_DEFINITION

This trace