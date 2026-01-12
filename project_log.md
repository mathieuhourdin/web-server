
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