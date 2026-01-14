
### 2026-01-17 

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