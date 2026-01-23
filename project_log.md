# Daily log file for the project
Every day I work on this project, I take notes of : 
- What i concretely do on the project
- What choices I made
- My choices if I make any
- My thoughts, hesitations and considerations on the project
- Some results of expermientations


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