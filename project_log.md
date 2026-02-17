# Daily log file for the project
Every day I work on this project, I take notes of : 
- What i concretely do on the project
- What choices I made
- My choices if I make any
- My thoughts, hesitations and considerations on the project
- Some results of expermientations


### 2026-02-17 TODAY FOCUS

I need to create the new pipeline today.
I can still delay the data migration I think. What i could use is : resource_relation between landmarks and resource trace for the reference. Maybe i create a reference entity v2 for this that persists in the resource_relation, with a json in relation_comment for now.

Then I should also work on the fact to put the user's Bio in a separated journal. What I need here is to choose the way the invariant 1 user - 1 Meta journal is respected, and to make the different traces have different roles in this journal.

Then I work on using this trace as the first trace analyzed for a new lens.

The next step would be to create the object reference pipeline step in the trace analysis pipeline.

Then we add the new extraction pipeline.

### 2026-02-16 New pipeline

The new pipeline should have the following steps

If first trace : create landmarks from bio. Actually I wonder if Bio shouldn't be a type of trace. But what would be this trace journal ? Bio journal ?

So the flow is : 
- Desambiguation from existing landmarks
- Extraction of elements
- Matching of long term elements -> where should it happen ? / Classification of the found elements

I need to work a little bit more on what to extract in landmarks to make the matching. The abbreviation list.
The trace mirror should also have relation to a list of landmarks.

Trace : Full text
|   desambiguation
v
Trace mirror : 
- Text with references desambiguation
- List of landmarks referenced (resource_relations, or reference objects). Reference objects could be : id, local_id, trace_mirror_id, landmark_id (could be empty ?), reference, reference_type (direct|description)

|
v

Is there a landmark matching / Creation here ?? Maybe. We should have a standalone matching / creation step I think.

| Extraction of elements
v

Elements :
- List of elements (Descriptive, Transaction, Normative, Evaluatives)
- Elements reference existing landmarks.

| Matching of elements with long term transactions ... The matching could be either on the landmarks or on some long term transaction entities. What long term entities ? Projects most of all : make progress in computer science, sociology project...
v


Doing the maximum of objects extraction and matching with the full text is good because I can rely on the full context. It should work if i have a limited size of trace.


### 2026-02-16 Next steps

- I want to work a little bit on the UI (make everything as clean as possible for demos to persons close to me).
- I could even make a small part of social in the UI, just for demo purposes : like a share button on elements, to post things like "I have discovered this new book I think it's great".
- I should finish the migration of data, and for that I need to work on a precise design for the analytical entities.
- Try to implement the new pipeline with desambiguation and referencing first, and then global extraction.

UI done.
Try to work on the workflow now. 



### 2026-02-13 Ideas about desambiguation

Abbreviations matching could work like this :
- We have a list of known abbreviations for references/landmark/objects (Reference entity could be what we need !)
- In a new trace, we look for all matches between all known references and text substring. We take those landmarks and add them to context.
- We also load a hot context with the landmarks referenced very often.
- We also may load existing references that have not found their landmarks.
- Then we make a desambiguation matching prompt.

Hot landmarks with exact match for abbreviations could be automatically matched without llm call but not sure it is a great improvement if we are going to do the call anyway. Maybe if we do this match first, then make the classification call that says what it is talking about, which could evaluate if we need more desambiguation, and then we process or not, with also filters for the the context with the classification result.

After desambiguation, we retrieve new references for existing landmarks (new way to refer to a landmark) and references with no landmark ("A fun book with abbreviation HP"). Then we can do deeper search (External/Internal RAG) to identify those, and create new landmarks.


### 2026-02-13 step point

I have a really good prompt for the extraction now.
What should I do next ?

1. work on desambiguation on first prompt.
2. Identify landmarks in the claims to reference them.
3. See how I persist the claims.
- They should be in elements
- Multiple types of elements
- Stop

lets work on desambiguation. It could help a lot for the rest of the pipeline.

### 2026-02-13 List of errors in the prompt

- Normative kind is not really defined -> Should have a reframe maybe.
- Normative span : should it take the transactions spans ? 
- Descriptive theme & span : should the theme span be something ? The whole sequence about this matter ? Including the evaluative / ... ? Should theme be a separate category ?

### 2026-02-13 Extraction evolution

Made a lot of work on extraction that I have not noted here.
I'm shifting toward a more gramatical extraction. Try to extract all sentences by unit of sens (claims).
I distinguish between DESCRIPTIVES, TRANSACTIONS (with INPUT, OUTPUT, TRANSFORMATION and QUESTION variants), NORMATIVES and EVALUATIVES.
This is works pretty nice, already better than the previous extraction only for resources / Input. However I still have some missed things, and I have a new level of expectations on the text analysis. So I want to do some more work on it.
What I need to focus on is : 
- No missed transaction
- Good identification of high level transaction and descriptives (projects...) that will help do some long term matching. Need to think about first pass on trace to add focus on such things.
- reproducibility

I could still try to implement the new extraction in the platform, at least to show the progress in the interface.

### 2026-02-12 Ideas for extraction

I have multiple approaches in mind to explore the extraction field.

- Take a given trace and write precisely what I expect to get. I need to know precisely what I want before to ask it to the LLM. Maybe try to make the request with this precise example then.
- Ask for multiple way to make the extraction to a LLM : Try it with only IO then with internals, with mixed IO or separated, with flatten target or not... Maybe I could use a different directory for those prompts. The risk with this approach is that none of the prompt would be good enougth if I don't work on them individually.

But first of all, I really need to work on what general output i expect from one trace. It should be paper and pen work I think. Ideally I would print those traces.
I think I also should test a trace chunking just to see if the extraction can do something good.

### 2026-02-11 Small point

- I start to have a nice interface
- I start to enter into the all kind of elements pipeline problem.

Need to keep on working on that now to have a very nice extraction of elements.

### 2026-02-11 Thoughts about the pipeline

I have a few ideas.
First there is the basic issue with abbreviations and very implicit references of often manipulated landmarks.

What I see as solutions : 

Abbreviations :
- We should identifie some often used abbreviations in the users traces (DDIA, MS) and store it as known_abbreviations 
- When a new trace is analyzed, we could look for landmarks with known_abbreviations in the user's landmarks.
- Then we replace the abbreviations with a short description of the landmark in the trace_mirror content. It could be some kind of encoding, to keep the abbreviation and have the explaination on hover in the interface.

Implicit references : 
- Most of the time, they are related to very "hot" landmarks (keep on reading the management book -> the is a good signifier of an implicite reference to a frequently seen book).
- We could use a desambiguation LLM prompt to make the reference to the object explicit.
- The general qualification call could tell us if we need to put them 

Actually I really need to have a high level projects entity type : the high level project for me currently are Matiere Grise, Mémoire, Find a job.
We also have the high level spaces. We could call them spaces actually. Like for me Sociology, Tech, Perso, Sport.
Maybe we should have a 


Training could be a given type of transaction. About a given competence. A training has a targeted_competence, an exercise, a volume, a level of difficulty... 


Just to remind me I have : 
I/O
- inputs : ingestion from resources. Could even be the food the person eats !
- outputs : production of artefacts. I don't want to continue the analogy.

Internals
- states : how i feel, competences evaluations etc
- trainings : exercises i make to change my competences
- behaviors : how I behave, the schemes in my activity (is it different from states ?)

### 2026-02-10 Tests with new extraction prompt

Extraction prompt with input, output and internals
It gives too much weight to the internals.
I should add a constraint like internals are only for references to self.

### 2026-02-09 Landmarks and transaction evolution

After thinking a little bit, it seems that landmarks should not be moving entities. Currently they are created by a first element, and then they are duplicated each time an element is related to them. However here we don't want the landmark to evolve in time : If they are stable entities such as resource, author, theme... they are fix entities.
What moves is the transaction. A user transaction with it's environment evolves in time (it can add some new resources to its context, etc.)

What could move for a certain time is the reference object that we could have : it could take some time to find the reference of a transaction. But then it is found with enough precision we should be able to know exactly what object it references even for old elements.


### 2026-02-09 On the extraction pipeline

A few reflections about pipeline model.

I have two kinds of traces : 
- Journal traces that are mostly relational : the user is strongly involved, and it talks a lot about its actions (I read this, I wrote that, I realized this about me...), with mostly timed actions.
- Note traces are about external objects, with few relation to the user. It's more like : this book is interesting for this theme, etc...

Then I think we should have two different prompts for extraction between those two pipelines. Then the rest of the pipeline could be quite the same to match landmarks with existing ones or to create a new one.
What should be different is if we create a long term transaction and we want to do matching with this. Transaction matching should not be run for the note pipeline branch.

One the other hand I have changed my mind for the journal pipeline : we should do altogether an extraction for the input, output and internal elementary events.
They have a similar structure : 

Fields of jounal extractions : 
- referenced_objects (resource, author, theme, deliverable...)
- verb
- status (done, intended)
- time_offset (-1, -2... (yesterday I did this))
- evidences, extractions...

Fields of note extractions :
- subject
- predicates (cites, recommends, critcizes, defines...)
- object (the resource...)
- qualifier (quote, chapter...)
- evidences / extractions


### 2026-02-09 Objectivs of the week

I want to make the mental model of the platform crystal clear for any user that gets in.
I also want to cover enougth of the traces content to have a big picture (inputs + outputs + selfs)
I want to make it clean. That means work on the onboarding (do i keep the beginning with missions ?), finish the database migration (what remain unclear in the model ? transactions and relations between landmarks I think.)
I may want to implement some checks/retries on the pipeline... to make it more like a professional AI agent pipeline.


### 2026-02-06 ROle of journals

Journals could have a very interesting role on being the integration support of distant journals : a google doc, a md file in a project, something in a github....

### 2026-02-06 End of the week status

I have added the refinement stage but I have not tested it yet (need to think about how to test such a stage).
Today I have made very big improvements
The pipeline is finally clear, with all the stages visible in the analysis_processor.


### 2026-02-06 Few ideas on transations 

Transactions should always have a verb. 
It is a verb (reading, watching... for inputs, thought, wrote, shipped... for output) with other static objects.

### 2026-02-06 Next steps 

What could I do now that i have a pipeline that works more or less fine ?

- Quality of existing stages of the pipeline : Checks on perfect matches, retries, timeouts.
- Introduce some RAG.
- Clean of the pipeline : remove the logger param passed everywhere, remove evidence and keep extraction (or ask for very short extracts/words for evidence, that could be used for the RAG)...
- Pipeline refinements : make new mentions of non identified landmarks trigger a gpt request for better identification
- Display refinement : display landmarks with their types in the analysis view, display links based on events, separate old landmarks and mentionned landmarks...
- Add the output / self reflexion pipelines.

This afternoon i can do : 
- Remove logger : ask codex -> Done
- change evidence for words extracts : Ask codex also maybe -> Done
- Alignement between mirror and elements extraction call : look then ask codex -> Ca a l'air bien en fait.
- Refinement pipeline stage : I could do it too -> Done
- Display type : Codex in front -> Done
- Mentioned landmarks vs last days context : allow a param in the get landscape landmarks route -> Done
- 

### 2026-02-06 Ideas about self reflexion

For the self reflexion part, I have a few ideas of categories.

- Moods / Emotions / Feelings : not in a good mood today, tired...
- Self Knowledges : I realize that I have hard times doing things with anticipation
- Self praxis : I do a review writing every morning.

### 2026-02-06 Global analysis on trace

At the trace level (To create a trace mirror) i think i should want to differenciate multiple things : 

I need to identify a trace kind. It could be something like : 
- Execution trace : list of many thing a user did.
- Reflexion trace : thoughts the user had about something...
- Reading note trace : summary of a resource or things like that.
- Introspection trace : things the user thinks about himself, self reflexion...

At the same time I should identify which part of the following pipeline to run.
For instance : 
run the input / resource extraction pipeline
run the output / deliverable extraction pipeline
run the inside / self reflexion extraction pipeline

Just thinking : inputs and outputs could somehow be some entities.
There could be some long term inputs (I have read HOM from the begining to the end, I did a long reading note and analysis on it) and short term inputs : I have read half an article from Bidet just to see what she talks about.

While resource / author / theme are objects clearly outside the user, inputs are more relational : they are the open transaction from the user to those objects in the Dewey sense.
Then I think that I could have a Transaction category for those objects (output and output)
Somehow, the user can be in a transaction with himself through the self reflexion transaction.

Ok I think I have found something really interesting. Let it take a little bit of maturation time before to think about how to represent it in the platform. But it should come at some point. 
In a way the current landmarks are public objects. A resource, an author, a theme, they should be shared among different users.
It is very clear how we can have public resources, authors. We could even think about a list of themes. It should not be that complicated to generate a list of themes actually. Then a transaction could have relations to those objects, with more or less precision in case those are not found (we could for instance have a list of three best match books for what the user has told in its traces).

The new objects I would have are : 
- References : they are references to public objects, more or less identified. 
- Transaction : an open (or closed) process between the user and outer entities / himself, entites identified through references.

References could play the same role as options in rust, some kind of a wrapper. It is really fun because it rejoins the theories we have in philosophie for reference, all the discussions about false descriptions references, do they reference anything etc...
A reference could have multiple states, locked-in, vague... Then the user could be asked to create a trace with desambiguation for the references we did not find while analyzing its work.

Ok all this is beautiful but first we need to implement the main part of the pipeline for the landmarks.

For now, I think we should see the transaction into the resource landmark type.

### 2026-02-06 TODO

Check the replay route : when all observability will be ok
Remove evidence part of extraction : could be done by codex
Use trace mirror context if it is a note : after observability
Try RAG : in another life lol

First of all I should make the pipeline very very obvious in the code for anyone looking at it (me first because each time I come back to it I am lost).
I should have one only method where I have all steps of the pipeline that I can see. It is the only way I could make it evolve easily (add new steps, add observability...)
I think what I can do is to have a file for each step (creation...) and a file tree for those steps but then all step is called from the main execution function.

### 2026-02-05 Done

I changed the pipeline organisation. We have a config, a context, an input, and a state objects, that are used everywhere in the pipeline.
### 2026-02-05 TODO

I need to check the replay route.
I want to check how the pipeline behaves step by step.
I may want to remove the evidence part of the extraction because it is redondant with extractions.
I want to use the trace mirror context if it is a note.
It would be really nice to try a little bit of rag.

For the log storing in different files for different analysis, I think I should think about different things.
- Maybe the analysis should be passed to every single function of the pipeline.
- For instance, every LLM calls are related to an analysis. So they should be related to an analysis, and in this way we could display them efficiently.
- LLM calls should have a system and user prompt because quite often I just want to look at the short user prompt. They also could have a name to make it easier to debug in general.

All of this is about observability. This could be my big work of today. I don't want to run my pipeline and not be able to observe what I do.

I have a doubt about something : should I pass a logger struct around, or should every function own the logic of logging from other inputs vars ?

### 2026-02-04 stage point

Did a lot of things today. Did not have the time to check on every of them if they were ok.

- Did not checked the replay route for an analysis
- Did not checked the ???
- I wanted to create separated log files for each analysis run in dev mode but did not
- There is somehow a bug that makes that the very first Trace is not run by the lens. (actyally it seems to be an import issue)

What I did : 
- Import of my own journals with a new bin.
- Rewrite of the existing extraction system prompt.
- Some updates on the front


Just had an idea : We have another invariant.
- traceMirror, elements, landscape, landmark are immutable elements for the user, through the API. They only have GET methods.
- analysis, lens are the way a user can do actions on the analytical part. They have POST, PUT and DELETE methods in the API.

### 2026-02-04 Fix to do
When there is an error in the signup process we need to make it clear for the user with an error message.
### 2026-02-04 RAG ideas

My pipeline with RAG could look like this : 
- extraction, just the way it is currently
- Internal search : find 3 records close to this one based on search key.
- External search : find 5 responses close to this one based on search key (title, author, theme).
- Matching : with this content, the external search result, do you think I should match to one of those 3 existing landmarks ?
- Creation : if not, generate a new landmark from the extraction and the search result.

### 2026-02-04 current state

I have completed the trace mirror creation, with a new trace mirror entity.
I have introduced a new entity_type on resources that helps differenciate between v2 entites, to prepare for migration.
I also splitted the v2 entities to make it easyer to use, but also to have a hydration file that holds everything that will be suppressed with the migration.
I have changed the trace_broker pipeline to be horizontally layered : 
- extraction 
- matching
- creation 

I have introduced two new landmarks : author and theme.
They are created during the same pipeline as resources, the input pipeline.

I also created a new 

My current pipeline runs, which is good.
However there is some issues / quality loss from previous one.
- In extraction, I take some elements that are not input (an email i write). I think this is because the new prompt is too long.
- In matching, sometime I dont match. Maybe I have a confidence threshold too high (0.7)
- Once a Landmark is created, it is never improved, while it could be (if y have a new element for this landmark, try again to identify the title. Maybe use some RAG at this point)
- Soon I will need more work on context and on RAG, to give something richer and more precise to model calls. Maybe the first thing to do would be to do it for the trace mirror pipeline.

What should I work on first then ?
- Extraction refinement
- matching threshold (immediate)
- batch import of my existing data to be able to test on richer data
- Tests on more data will also require more work on analysis control to be able to run / reverse / rerun on a single analysis / trace, so that it will be easier to check how it performs and how the pipeline improves.
- Some researches on how I could use RAG in my pipeline, to think about a little POC somewhere.


(resource could be information_vector)

I thought about RAG in different ways : 
- Internally, I could request for close landmarks using fuzzy search. It could help retrieve a better context thant all related landmarks. It is not clear how it helps however, because : matching is performed on multiple elements at the same time.
- External : The obvious way is to query an internet search engine before creation to help identify the exact resource.

I'm thinking about something : maybe the extraction would perform better if I splitt the trace in smaller parts.

### 2026-02-03 Today

I need to create a TraceMirror entity because it is different than a element.
Then i will be able to use the mirrors in the following pipeline.

### 2026-02-03 Next steps

Now I have an analysis for the TraceMirror. 
I should use this trace mirror for the rest of the pipeline, in order to have all the context informations about the trace (is it a note trace, what is it mainly talking about...)
It should now be the time to add a new landmark, and I think it should be theme. Or people. I dont know. 
But this raises the question about how to design more precisely the elements. Should an element be linked to two landmarks ? Recently I have found that asking for a trio reference, author and theme for the extraction was working really well, because it is a natural mapping to many resource references, and there is almost always one or two of thoses three mentioned when a resource is referenced.

What came to my mind is the following thing : 
- I could reuse the input / output breakdown that was first in the platform
- The user's activity is a flow of inputs he takes from the outside and of outputs he provides.
- That flow is what we organise and make stable using landmarks
- In a advanced design, we should link inputs to outputs but in a V1 we can treat them as independant
- Inputs are trio resource, author, theme that feeds the user work
- Outputs are mostly a duo idea, deliverable. Or action, deliverable
- A third category could be all the qualitative things the user says about himself (I am tired, happy...) that we could store as raw materials for now.
- Thinking again, we should have a big partition of the user's space between different aspects of his life (perso, tech, socio...). However this is strongly related to my own multi activity and shouldn't be a first priority. At least, put some tags on the trace mirror to help retrieve the right traces when we want to perform the analysis.

For now, what matters is to focus on the input part. That is the easyest part I think.
This means to work on the trio resource, author theme.

It seems clear that a single element should be allowed to belong to multiple landmarks. We want to know that a part of a trace uses the resource A and is about B, not to know independantly that there is a part about A and a part about B.
The author could stay aside for now I think, because they are strongly related to the resources. We will discuss this later.
But the first thing is that we have a n-n relation between elements and landmarks (or maybe multiple 1-N ? I should think about that later.)
Then in the pipeline, the extraction should be performed with both theme and resource.
We could have a single extraction run, and then two matching runs for the matching.

I dont see any issues here actually.

What is more complex is the following : when I have qualified the whole trace, what should I do on the elements from the trace ? Should a link them to the global resource each time ? But if it is a resource mention, then it would be related to two resources ? Maybe we should have a hierarchy : a resource as global context, another as local focus. Same for the theme. I think that, for now, it is better to think about it as :
- The trace mirror holds the global context
- The element holds the local information.

In a note, some parts will be mentions of other resources and will be mapped to them.
But some other parts are just from the gloabl resource, but treat sub themes of the global theme. Eg in a encoding related trace, we have an element about avro, an element about protobuf.
There is really no problems about that. We just should do some experiments to see what prompts and pipeline give the good result but it should work.


### 2026-02-02 Work of the day

Today I have made some good improvements.
Now the trace mirror creation is working. I have made the extraction, matching and creation.
I made the matching part more abstrait and this is a good thing, i will be able to use it for all future matching problems.
I think I should work on a similar structure for most LLM calls : when I call the LLM for a collection, I dont want to ask it to repeat the existing fields, I just want new fields and to gather the information after that.

I worked on the PUT lens route too. I will need some more things to prevent going back in time. 

Howerver now all of this is working pretty good. I will need to work deeper on themes and to fine tune my pipeline but then I think it will be quite ok.

### 2026-02-02 Trace extractions

I have made a new experiment where I extract a vector of evidences from the trace to justify what part are referencing a resource. Then I make sanity check to verify that the evidence is really a substring of the existing trace.
I should think about two kind of extraction : 
- extract a small piece of the trace that justify the association. We want to keep it small, for instance a simple extraction of the title, or author + theme.
- extract all the text that is related to this extraction. This is how we make sure to have all the contained information from the trace for the following analysis.

For the note tracemirror analysis, I think I can stick to the first part because the whole trace is supposed to be about the same resource.
However I can imagine more complex situations : 
I have a trace that is a note about a resource. However the user has an idea during the book reading. Then the trace has both a summary part and a new idea part. But I think this should be treated later in the pipeline.

### 2026-01-30 move toward v2 and new pipeline

I worked on the existing pipeline to remove all use of legacy api. 
Now I only use the v2 entities. 

I also refined the pipeline orchestration. And I started to create a new subpipeline for the Trace Mirror.

### 2026-01-29 Small Idea

I could use a search pipeline on the traces to find matches on some words, and to look for known abreviations etc. When an abreviation is found, i could replace the abreviation by the content of the matched landmark. I could have it for the proper names too. Like if i find a book title... Don't know if it's a good idea to do it before analysis, I should try it.

### 2026-01-29 Naming of the first element analyzed from a trace

I should find an element name. Options : 
- FirstElement : always the first element created when we analyse a trace. It reflects the constraint that the following analytic elements belong on this fondational element, and that this element should never be deleted if other are created in the same landscape_analyis.
- AnalyticTwin : a twin of the trace. Ok, it makes me think about digital twins, but somehow it doesn't reflect the fact that it is part of the analytic world. Not as much as TraceMirror.
- TraceMirror : it is a mirror of the trace in the analytic part of the platform. This has the advantage of keeping the optical/perception metaphore we have in the rest of the platform.

I should have a constraint on the elements : Unique trace_id, analysis_id where kind = FirstElement.

A question is : should the trace mirror be referenced by the next created elements ? It is a good way to make it explicit that this element is fondational. The fact that I shouldn't delete this element before others is important. I may want to replay the end of the analysis but not the pre analysis pipeline.



### 2026-01-28 Thoughts about new pipeline

I had some thoughts about the new pipeline.
Maybe it should not be a new pipeline.
It should just be a first stage in the existing pipeline. Why ? 
When we analyze a note trace, there is a lot of work to do that is very similar to the journal trace analysis. Eg. the search for resources cited in the trace. Because a lot of notes will reference bibliography.

So what we really need to do is to think deeper this first stage of analysis we already have when we persist the new trace.
I think the trace mostly should have some metadata from a first anaysis. 
And those metadata should help the Landmark processing the trace by the following pipeline.

Currently the pre analysis of the trace is not in the pipleine. It runs synchronously when I persist a new trace.

However it is somehow anti-pattern to do so. All LLM created data, not validated by the user, are supposed to belong to the analytic entites (Lens, Landscape, Landmark, Elements). 
Howver we need to think a little bit about that. It has some frictions with how some parts of the app is designed now

Currently I use those generated content for : 
- I display the traces with their title and a substitle. 
- And I create a interaction_date that equals to the date they were really written at (not the date they are added to the plateform, for imports cases)

Title : if the title is now in an other entity, related to the analysis, I wont really be able to use it for display purpose. Because it is not easy to get the related analytic element for a trace, because there could be multiple analysis run on this trace, etc. Actually it could work if the title is really independant from the previous context of the analysis. Then I could retrieve any element related to this trace and use the title to display.
Interaction date : Actually I should just work on an import pipeline from traces / journals the user already has. Some kind of ETL pipeline. 

Solution that seems ok : 
- Allways keep the trace as it is first. Raw content, the user gives it a structure in the raw text if he wants but no more.
- The first stage of the process creates a first analytic element. This element does content extension for the trace. It gives a title, a subtitle maybe, maybe some tags. It could work on desambiguation from the full context of the user, or from an abreviation list idk. All this could make it a mirror of the given trace, ready to be indexed in a search index.
- This first element could be related to some landmarks, as global landmarks for this trace. Resource DDIA if the trace is a note about DDIA. Theme DB and resource Chatgpt if it is notes from questions to chatgpt by the user (or a full response !)
- Then the pipeline keep processing with normal stage, but can rely on the existing context, and creates default relations to this resource and this theme.


I have a small concern about the link between analytics and lenses. Lenses are the main way to change analytic context, in the graphical interface but it should also be the case in the api. For instance if i want the first analytic element associated to a trace, I should pass the lens ID, or my current landscape analysis.
But currently the lens has only a link to the head analysis. Analysis cant have a direct unique link to the lens because with the branches, an analysis can be an ancestor to multiple lens. 
Just thinking, a solution could be to tag all ancestors of a given lens with the lens uuid, or with a has, or idk, to be able to retrieve this in a easier way than just recursive search. I dont know the classic solutions for that kind of problems.
I see another way with a n-n relation between lens and landscape analysis, but i'm not sure it is a good idea. A lot of relations for something that is already expressed in the model, and it will need many joins to do it. 


Just thinking : I should make the distinction of all those big parts of my system appear in the architecture : 
3 blocs : 
- UserEvents -> Content created by the user, Journals and Traces, the source of truth of the system , 
- Analytics -> What is created by the analysis pipelines. Could run multiple time. They are projections in Event Based langage.
- Publications -> What the user shows of its work. Deliverables he shooses to publish, traces... All this is user validated even if some parts could be AI written.
- System for other entites such as sessions, LLM calls... 

### 2026-01-27 Results

I made some new things on the frontend graphical representation of the platform : 
- A list of traces ordered by date
- An identifier to show where the current landscape_analysis is sitted in the trace list
- The possibility to move to former lanscape_analysis in the parent list of the current landscape_analysis
- It shows updates in the displayed landscape

This uses some new routes in the Rest API, to recursively retrieve the parents of the current lanscape.
The recursive implementation is really naive currently but it is ok for the current depth of the trees. But at some point I will want to use some more efficient request such as with recursive.

I have started to think about the implementation of the Note pipeline.
The stages of a v1 of this pipeline seem to be quite straightforward.

The more interesting part is how we do a splitting in the note trace. I want to try a theme split.

### 2026-01-27 About the reading notes pipeline

I could have a pipeline that manages two specific types of traces : 
- Journaling traces. Very diverse, multiple subjects and resources, maybe some "noise" about small things of the day...
- Reading notes traces. All about the same resource, even if some links could be done with other resources/questions/tasks.

The first stage could be to identify the type of trace we are talking about. I see multiple options : 
- The pipeline recognizes only based on the trace what kind of trace we are talking about.
- The trace type is recognized from the description of the journal the trace belongs to
- The journal has a type that is infered once for all from the description of the journal
- The user chooses the type of journal in a list of choices.

I think that for a first experiment we could have a user choosed journal type because it avoids creating a new pipeline stage.

Another question arises : 
Do we want to store the whole trace in the resource landmark or to split it in small pieces ?

To have a really enriched content, we would love to have 
- a trace
- in a trace, a split of multiple elements that are linked both to the trace and to a specific theme.
This way, we also have a graph of the different theme that appears in a given resource. We could present the resource from different point of view : chronologically, or by theme, etc. It is also a nice way to link themes to resources. It is also a way to test a theme identification pipeline.

A pipeline for such a trace would look like : 

- Identify the resource it is talking about
- Search the existing resources to see if any title matches (be careful about abreviations such as HOM, DDIA)
- Create a title, subtitle
- analyze the existing themes

### 2026-01-26 Thoughts about next steps

Several next steps are possible. I should also consider that I need to show something in production quite quickly.

Options : 
- work more deeply on resources identification. Be sure to identify all resources in a trace etc. Maybe implement some RAG for better identification with Tavily.
- Extend the trace analysis for other landmarks : task and/or deliverable.
- Do the full switch for the new entity model (think about how to keep the existing written articles display)
- Do some cleaning on some parts of the platform : auth, api response format, CI/CD, maybe some tests.
- Extend the analysis toward different kind of traces. More precisely, create a specific pipeline for traces that are about a unique subject (note taking about a book for instance). This is a specific use case that could be my real MVP, and probably far easier to implement than the task following part. It could require to give more knowledge to the journal entity (it could hold some global parameters for the traces it holds, such as this is a journal for traces that are notes about books). This way the analysis pipeline could be much more precise and don't load all the context each time (if we are sure that it is a trace about a book, we should retry the analysis until we find the referenced resource, do some RAG, or even ask the user if nothing is found)
- Work on the public display of the plateform (a SSR part with a link to the SPA part, and maybe just a given page for the preview of each article)

### 2026-01-23  Work on the landmark API and pipeline prompts

I created a route to get a landmark with all its parents landmarks, and the elements of its parents.
This way we can get all the history of a given landmark.


I also worked on the pipeline. 
- Removed the already existing resources from the extract prompt to avoid false positive
- Created a new prompt with examples of the desired outputs.
- I removed the too custom examples (Ughetto in the prompt while my traces are about him)
- More distinction between resources I consume and resources I produce
- Desired but not read resources 

It works better I think : have no false positive (repeated Ughetto mentions).
I have also made some tests with stronger models (first I thought I was using 4.1-mini but actually I was still on nano)
- It gives really better results with mini
- base 4.1 creates big costs to run the full pipeline, with results ok but not far better than mini.


I also made some researches about RAG implementation in my pipeline.
I could use searches using Tavily.
Maybe it would fit after the extraction and the matching : if a resource is not properly identified, make a search query to have more things about it. Or even before the matching I dont know. Need to make some tests.

### 2026-01-22 Work on full-text context and matching

I created a track for a full-text context summary of the user work. It gives some ok results but tends to repeat some information. Maybe it could be better with a stronger model.

I also added a non LLM matching step in the pipeline. If an extracted element has the same title of an existing resource, it will be matched automatically and the data is not sent to the LLM.

### 2026-01-21 Finished matching

The lean matching is working now. We dont ask for the whole entity but just for a matching using int ids.
Matching could become a more standealone feature of the pipline at some point maybe. But for now we keep it like that.

Lets focus on the next refinements in the pipline : 
- dont pollute the ontology with empty landmarks 
- make the extraction work on the whole trace and don't miss some references to resources.

### 2026-01-20 Work on lean matching

I want to give local ids to the elements and the landmarks that I match in the matching process. 
It requires to think it well to have a simple way to create those local ids, and to do the matching next.

I think i should have a struct like local array.
The elements would be something like : 
struct LocalArrayItem<T> {
local_id : i32,
    item: T
}
with T se / de.
Then we have a method that takes a vec of items, and creates a LocalArray.

We have a method that serialize the item the way we want for the matching (flat structure).
We do the same for the two arrays.

Then we send this to the LLM.

After that we use a method that will reconstruct the desired matched elements from the two localArrays

### 2026-01-20 Work on the prompts for the pipeline

I have some troubles with the prompts i use. I have solved some issues, such as lost landmarks in the analysis process.
However now I have other problems : 

- Creation of unidentified landmarks that pollute the pipeline in the following steps (resources with empty title and not enougth data to identifie resource)
- Sometime not every resources are identified, the model focuses on specific parts of the trace and forgets about the rest.
- Sometime the process creates resources like "Chatgpt and databases", which we don't want. It is more like a concept or a thema. 
- We somehow need to refine our ontology because we have some troubles defining if something should be a resource or not. Eg if I use chatgpt to make some research on a topic, should chatgpt be a resource ? Should the topic be a resource ? I think we could have a topic landmark type, and a tool landmark type. Maybe a LLM should be in the same category as reading (we want to know what percentage of the information the user gets is from reading and what percentage is from LLMs)
- The matching step asks things we don't need, such as repeating the title... we should only ask for a match of ids and maybe a confidence. Then we just rebuild the full element from the matching.

### 2026-01-20 Work on lens and landmark_analysis

Last days I have made some work on lens creation, deletion and landmark_analysis processing.
It is now possible to create and delete a lens, and that will create landmark_analysis for each trace between fork and target.

This basically works now.

However there is still some cleaning that needs to be done : use the v2 entities everywhere in the work_analyzer, remove some code that is not used anymore.
I also should be carefull about cleaning the graph if there is an error in the processing.

### 2026-01-16 next steps

I have made some moves in the direction of the new model.

I created the Lens entity (still not sure Lens is better than AnalaysiBranch)
We will need a few routes for the lens entity :
- Post : create and run a new lens between begining / fork landscape and target trace.
- PUT : run the existing lens from its current landmark and the new target trace.
- GET : get all the different lens to be able to switch
- GET one : get the current landscape of the lens.

The analysis entity should be modified too. I should rename it to Landscape (still not very happy to have state and command in the same entity without having the name indicating it. I may want to change that)
- Renaming
- Remove the logic that should now belong to lens (post analysis)
- add some logic to retrieve a landscape from resource, and to hydrate with user_id, parent_landscape, analyzed_trace.
- Use the analysis/landscape entity in the analysis processor instead of resource


When i create / extend a lens, will want to plan some analysis jobs, landscape creation. What seems the best way to do it to me is : 
- get the previous landscapeAnalysis
- get all traces between current/fork/begining and target, ordered by date
- from the current/fork/begining trace to the target, create a landscapeAnalysis, with the previous state at parent, and the processing state at notstarted. We return the response to the client at this stage.
- then spawn some threads of analysis for each landscapeAnalysis. An analysis can only start if it parent processing state is completed. We loop / sleep(0.5s) until the parent state is completed, then proceed the analysis. We should catch every error to unsure that if something goes wrong we set every states to error and stop processing of child analysis.

I wonder if anything can go wrong if i spawn too many threads for the different analsyis jobs.

If the analysis is from the begining, we should first create a landscapeAnalaysis based on the user's biography

Implementing a incremental id for traces by user should be usefull quite rapidly too.

### 2026-01-16 About analysis api

Other ideas on analysis.
I think I should have two entities related to analysis.

One is analysis and the other one is analysis_run, or analysis_play

analysis_play records a request to play / replay analysis from a certain point in the system.
It has a field analysis_start_id that points to a unit analysis or a unit landscape. 
It also has a field date in case we dont want to play the analysis until present.

When we create an analysis_play, we find the parent analysis, and we start running unit landscape analysis from this date.
We get the previous landscape, and for each trace existing we create an analysis, then we process the analysis and create a new landscape based on the trace. We loop through traces until the given date (or today). 

This rises some questions for me about how to ensure that every traces are fully processed when we process a new trace, to avoid race conditions on traces analysis. Maybe this is a usefull purpose for an analysis entity separated from landmarks. In this case we can create an analysis linked to its parent before to process any analysis, and then process analysis in the right order.

Maybe we could need an analysis branch concept, just like in git. Is it different from a analysis_play ? maybe it could be the same, and if we want to just run more traces for a given branch we PUT/PATCH on its date.

Another question is : do we have an ordered structure for the traces ? Should we only consider traces as independant events only indexed by their date, or should we think about it with some kind of causal structure, that should enforce things like consistent prefix reads ?
While in git the commits, the events the system is based on, are the one that branch. However here we have a non branching list of traces. Traces are somehow how time is defined in the univers of the user work. Good question if we should refer to a certain point in time through a date or through a trace id. It also raise the question of cases where someone wants to add a trace in the past, at a date where further traces have been analysed already (eg. i want to import some traces from another system). Open question too.

I think it is a good idea to have a analytic_branch concept. And the user should be able to switch branch to see different analysis branches. An anaysis cant have an analysis branch because it can belong to multiple branches, however the analysis_branch can have an analysis it points to. It could have global settings, such as autoplay or not (do i want to run analysis each time i create a trace or to be manually used), the type of model i use for this analysis...

Actually we will call it lens to make it very clear that it is about how we see the objectiv nature of things that is contained in the traces.

### 2026-01-16  About landscapes

New thoughts about the object model.
I think I should use the landscape word somewhere. It fits very well with the landmark word. A landscape is a collection of landmarks. They are found / built in the flux of elements that occurs during the environment exploration.
In this view a landmark is quite similar to the current analysis entity.

I view multiple options.

A landscape could be the current analysis, and the analysis could be more an analytic content produced by the plateform (it could be included in the landscape as the high level analysis, or produced over a period of work, not just a trace).

What we have now is landscape_n1 = landscape_n0 + trace or landscape_n1 = f(landscape_n0, trace).
The landscape are naturally linked together. You can picture someone walking in the nature, and the landscape is evolving while the person moves. It is possible to think that landscape are linked together.

However we still should have a kind of entity that refers to the analytic process of producing a landscape from previous landscape and new trace. This entity could hold some informations about the type of processing : the version of the pipeline used to process the trace, the cost, anything like that that helps observability (a little bit like llm_calls but for the whole pipeline). This could be the analysis. We could have an analysis type for the landscape_trace processor, but also for things like week analysis. The analysis is the operator and the landscapes are the before / after states. It needs more thoughts.
We would also have an analysis created for analysis that run on a full week or a full day. It would be another type, corresponding to another pipline of data processing.

As a result, we could have the analysis as the event command, and the landscape as the updated state from the trace event.
Analysis plays the role of a job for the processing.

### 2026-01-15

I worked on the landmark entity. I added some fields on the NewLandmark entity to match the futur shape of the entity & db table : A landmark will have a n-1 relation with analysis, a n-1 relation with user and a n-0/1 relation with parent landmark, so I created 2 not nullable fields and one nullable on the struct.
It will replace the current n-n relationships supported by resource_relation and interaction tables.

Some thoughts about the data model.
An analysis has a parent analysis (we continue the analysis from a certain point of analysis, which mush hold all the state of the analysis).
However I see two models for the analysis to hold the state (the related landmarks).
The elements are a easy entity : they are only related to the trace they are extracted from, and related to the analysis that produced them.
On the opposit the landmarks are shared between analysis, because they must persist through time.
The prefered method is to create a new analysis with reference to its parent each time the analysis is mentionned in an element. No need to duplicate a Landmark when it is not mentioned anywhere (or maybe by some kind of a garbage collector but this is out of current focus).
But then how do we decide which landmark correspond to the current analysis context ?
First option is to decide that an analysis context is all the landmarks belonging to its parents. 
The other way is to say that an analysis holds references to all the landmarks of its context, even if the landmarks have been created by a previous analysis. The pros of this model is that it is immediat to retrieve all the landmarks of a given analysis, it avoid using some kind of recursive retrieval. The cons is some overhead of complexity in data model : we should have a n-n relationship between landmarks and analysis, and we should have strong logic on landmark deletion (only delete landmarks if no analysis point to it, or if the parent analysis (the one that created it) is deleted).
I note that this is the git model (a commit stores references to all current files of the repo but only ownes files modified at this commit)
I have no real preferences at this stage. Let's see.

One strong invariant I have in any option is that an analysis and its owned landmarks cannot be modified / deleted if the analysis has children.

After some LLM chat, it appears that the second option is the best.
The argument that settles the question is that after a few analysis the recursive retrieval of the context will take a very long time, and based on the expected amount of recorded traces and related analysis, the size of branches will be very long.

We will choose option 2 then.

A landmark could have a owner_analysis_id, and the landmark is deleted only if this analysis is deleted. We could enforce other constraints like to check if other analysis reference it but the base invariant about no delete on analysis if it has childs should prevent that.


### 2026-01-14

Today I will first work on the global execution of the pipline.
I want to run analysis trace by trace to have a more atomic run.
But we keep one call on analysis to trigger all the run on a given time window.
The created analysis represents a run. It creates child analysis for each trace on the given time window.

I have multiple design choices. I could allow multiple analysis_run for each user. 
One user creates an analysis_run for a given date and the system produces an analysis linked_list from this analysis_run, with an analysis for each trace. This will allow multiple analysis_run to coexist at the same time (eg. if we want to test the pipeline with different model etc).
Then we should be able to switch analysis_run view in the api to retrieve a different landmarks set depending on the analysis we request.
The question then is how do we avoid running the analysis for every traces since the beginning each time we launch an analysis run (sometime we want to just pursue the current run, or to replay from a given point). I think the easier thing would be to be able to give a parent_analysis in the post analysis_run to indicate a previous state from which we want to replay the analysis. 
One first thing to do in this direction must be to create an endpoint / a method to get the all context associated with an analysis. This way i could really have a function that takes a parent analysis and creates a new analysis from the previous state. At some point i would also want to create ways to visualize all of this in the API/Interface to inspect what has been created.

For this method get_analysis_context, it seems that i should loop through the parent analysis and retrieve each time the related landmarks if they are not parents of a landmark of the previous landmarks.
This is a more general problem : since i choosed a linked list pattern for many situations where I want to keep an historic of the entity, I should decide how i implement the reconstitution of the global and current state.

### 2026-01-13

Today i want to finish the new pipeline so that i can test it.

The new LandmarkProcessor was finished. It was tested against some traces and it works ok.

### 2026-01-12

Today i want to continue creating and testing my new pipeline. I should try to implement the same pipeline for tasks and deliverables.
Ideally I should create an abstraction layer for the landmarks maintainance.

I have made some search on how i should implement this abstraction layer. I should instead a ProcessorConfig struct and a LandmarkProcessor trait. Then a processor for each landmark type that will implement the trait.

### 2026-01-09

Today I try to implement a new version of the analysis pipeline.
The landmarks in a trace are not analysed altogether but type by type. And we split the new trace only regarding the lanmark type.
We ask first : in this trace, do you see any reference to a resource ? If so, identify and extract the part of the text that talks about this resource. Identify the resource. Check if it belongs to this list of resources currently explored by the user. If yes, give the resource id. If not, create au new resource.
Then we do the same for task, question...

I think this way we can keep a very small context for each request.

Result : 
It worked quite well this new way.
The pipline is more or less working for the resource landmark.
I should implement again all the pipeline based on this idea.

I also started implementing a new internal API around the concept of Landmark.
For now it retrieves and persist datas using the Resource API.
Latter I will switch to a real peristence for this API.

I will do the same for journal, trace, element, analysis.
It would also be a relief to replace the resource_relation entity by direct references between entities.